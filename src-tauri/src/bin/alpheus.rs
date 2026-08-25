//! Standalone Alpheus CLI, Interactive TUI, Analytics, Dupes & Growth Diff.

use alpheus_lib::analyze;
use alpheus_lib::disk;
use alpheus_lib::dupes;
use alpheus_lib::exec;
use alpheus_lib::history::{self, HistoryEntry};
use alpheus_lib::scan::{self, ActionKind, Card, Tier};
use alpheus_lib::snapshot;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{self, Clear, ClearType},
};
use serde_json::json;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::{self, stdout, Write};
use std::path::Path;
use std::process::Command;

fn print_help() {
    println!("Alpheus CLI — Storage Manager & Cleanup Engine (Linux / Omarchy & macOS)");
    println!();
    println!("USAGE:");
    println!("  alpheus [COMMAND] [OPTIONS]");
    println!();
    println!("CORE COMMANDS:");
    println!("  scan              Scan disk and display reclaimable categories (default)");
    println!("  interactive, -i   Launch the interactive terminal TUI menu");
    println!("  clean <id> [-y]   Reclaim a specific category with confirmation");
    println!("  clean --all-safe  Reclaim all safe-tier categories automatically");
    println!("  dry-run <id>      Preview exact paths and bytes to be reclaimed for a card");
    println!("  status [--json]   Show disk summary, free space, and JSON breakdown");
    println!();
    println!("ANALYTICS & TOOLS:");
    println!("  top [dir]         Find top 20 largest directories and files (default: $HOME)");
    println!("  dupes [dir]       Find duplicate files with multi-stage SHA-256 matching");
    println!("  snapshot [dir]    Take a disk usage snapshot baseline");
    println!("  diff [dir]        Compare live disk usage against the last snapshot");
    println!("  schedule          Manage background automatic cleanup timers (systemd / launchd)");
    println!("  completion <sh>   Generate shell auto-completions (bash, zsh, fish)");
    println!("  history           Display the log of previous cleanup actions");
    println!("  help, -h, --help  Print this help message");
    println!();
    println!("EXAMPLES:");
    println!("  alpheus                   # Quick overview scan");
    println!("  alpheus -i                # Interactive keyboard-driven cleanup");
    println!("  alpheus top ~             # View largest space hogs in home folder");
    println!("  alpheus dupes ~/Downloads # Find duplicate files in Downloads");
    println!("  alpheus diff ~            # Track disk growth since last snapshot");
    println!("  alpheus clean --all-safe  # Safely wipe build caches and packages");
    println!("  alpheus schedule enable   # Enable weekly automatic safe cleanup");
}

fn fmt_kb(kb: u64) -> String {
    let gb = kb as f64 / (1024.0 * 1024.0);
    if gb >= 1.0 {
        format!("{gb:.1} GB")
    } else {
        let mb = kb as f64 / 1024.0;
        if mb >= 1.0 {
            format!("{mb:.0} MB")
        } else {
            format!("{kb} KB")
        }
    }
}

fn fmt_delta(delta_kb: i64) -> String {
    let sign = if delta_kb >= 0 { "+" } else { "-" };
    let abs_kb = delta_kb.unsigned_abs();
    format!("{}{}", sign, fmt_kb(abs_kb))
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn emit_json(cards: &[Card], usage: &disk::DiskUsage) {
    let safe_kb: u64 = cards
        .iter()
        .filter(|c| c.tier == Tier::Safe && c.action != ActionKind::Explain)
        .map(|c| c.size_kb)
        .sum();
    let care_kb: u64 = cards
        .iter()
        .filter(|c| c.tier == Tier::WithCare && c.action != ActionKind::Explain)
        .map(|c| c.size_kb)
        .sum();
    let manual_kb: u64 = cards
        .iter()
        .filter(|c| c.tier == Tier::Manual || c.action == ActionKind::Explain)
        .map(|c| c.size_kb)
        .sum();

    let reclaimable_kb = safe_kb + care_kb;
    let free_gb = usage.free_kb as f64 / (1024.0 * 1024.0);
    let total_gb = usage.total_kb as f64 / (1024.0 * 1024.0);
    let free_pct = if usage.total_kb > 0 {
        (usage.free_kb as f64 / usage.total_kb as f64) * 100.0
    } else {
        0.0
    };

    let obj = json!({
        "version": env!("CARGO_PKG_VERSION"),
        "disk": {
            "free_kb": usage.free_kb,
            "total_kb": usage.total_kb,
            "free_gb": free_gb,
            "total_gb": total_gb,
            "free_pct": free_pct,
            "free_formatted": fmt_kb(usage.free_kb),
            "total_formatted": fmt_kb(usage.total_kb)
        },
        "summary": {
            "safe_kb": safe_kb,
            "care_kb": care_kb,
            "manual_kb": manual_kb,
            "reclaimable_kb": reclaimable_kb,
            "safe_formatted": fmt_kb(safe_kb),
            "reclaimable_formatted": fmt_kb(reclaimable_kb)
        },
        "cards": cards
    });

    println!("{}", serde_json::to_string_pretty(&obj).unwrap());
}

fn run_scan(cards: &[Card], usage: &disk::DiskUsage) {
    let free_gb = usage.free_kb as f64 / (1024.0 * 1024.0);
    let total_gb = usage.total_kb as f64 / (1024.0 * 1024.0);
    let free_pct = if usage.total_kb > 0 {
        (usage.free_kb as f64 / usage.total_kb as f64) * 100.0
    } else {
        0.0
    };

    println!("\x1b[1m══════════════════════════════════════════════════════════════════════\x1b[0m");
    println!(
        "\x1b[1;36m  ALPHEUS STORAGE MANAGER\x1b[0m  v{}",
        env!("CARGO_PKG_VERSION")
    );
    println!("\x1b[1m══════════════════════════════════════════════════════════════════════\x1b[0m");
    println!(
        "  Disk: \x1b[1m{:.1} GB free\x1b[0m of {:.1} GB ({:.1}% available)",
        free_gb, total_gb, free_pct
    );
    println!();

    if cards.is_empty() {
        println!("  \x1b[32m✔ Disk is clean — nothing left to reclaim.\x1b[0m");
        println!();
        return;
    }

    let tiers = [
        (
            Tier::Safe,
            "\x1b[1;32m[SAFE TO RECLAIM]\x1b[0m",
            "Regenerable build caches & package artifacts",
        ),
        (
            Tier::WithCare,
            "\x1b[1;33m[NEEDS A DECISION]\x1b[0m",
            "Removable, but review target list before deleting",
        ),
        (
            Tier::Manual,
            "\x1b[1;37m[MANUAL / INFORMATIONAL]\x1b[0m",
            "Information only",
        ),
    ];

    let mut total_safe = 0u64;
    let mut total_care = 0u64;

    for (tier, heading, sub) in tiers {
        let group: Vec<&Card> = cards.iter().filter(|c| c.tier == tier).collect();
        if group.is_empty() {
            continue;
        }

        let group_sum: u64 = group
            .iter()
            .filter(|c| c.action != ActionKind::Explain)
            .map(|c| c.size_kb)
            .sum();

        if tier == Tier::Safe {
            total_safe += group_sum;
        } else if tier == Tier::WithCare {
            total_care += group_sum;
        }

        println!("  {} — {} ({})", heading, sub, fmt_kb(group_sum));
        println!("  ──────────────────────────────────────────────────────────────────");

        for c in group {
            let size_str = if c.size_kb > 0 {
                fmt_kb(c.size_kb)
            } else {
                "—".to_string()
            };
            let action_tag = match c.action {
                ActionKind::Delete => "\x1b[31mdelete\x1b[0m",
                ActionKind::Command => "\x1b[35mcommand\x1b[0m",
                ActionKind::Explain => "\x1b[90minfo\x1b[0m",
            };

            println!(
                "  \x1b[1m{: <24}\x1b[0m  {: >8}  [{}]",
                c.id, size_str, action_tag
            );
            println!("    └─ {}", c.title);
            if let Some(cmd) = &c.command_display {
                println!("       \x1b[90m$ {}\x1b[0m", cmd);
            } else if !c.paths.is_empty() {
                let p = &c.paths[0];
                let extra = if c.paths.len() > 1 {
                    format!(" (+{} more)", c.paths.len() - 1)
                } else {
                    "".to_string()
                };
                println!("       \x1b[90m{}{}\x1b[0m", p, extra);
            }
        }
        println!();
    }

    println!("\x1b[1m──────────────────────────────────────────────────────────────────────\x1b[0m");
    println!(
        "  \x1b[1;32mReclaimable:\x1b[0m {} safe  |  {} total",
        fmt_kb(total_safe),
        fmt_kb(total_safe + total_care)
    );
    println!("  Run '\x1b[1malpheus -i\x1b[0m' for interactive mode or '\x1b[1malpheus clean --all-safe\x1b[0m' to clean.");
    println!();
}

fn run_dry_run(card_id: &str, cards: &[Card]) {
    let card = match cards.iter().find(|c| c.id == card_id) {
        Some(c) => c,
        None => {
            eprintln!(
                "\x1b[31mError: unknown card '{}'. Run 'alpheus scan' to list available IDs.\x1b[0m",
                card_id
            );
            return;
        }
    };

    println!("\x1b[1mDry-run for category: {}\x1b[0m ({})", card.title, card.id);
    println!("Description: {}", card.description);

    match exec::dry_run(card) {
        Ok(dr) => {
            println!("Action Method: \x1b[1m{}\x1b[0m", dr.method);
            if let Some(cmd) = &dr.command {
                println!("Command to run: $ {}", cmd);
            }
            if let Some(w) = &dr.warning {
                println!("\x1b[33mWarning: {}\x1b[0m", w);
            }
            println!("Total Reclaimable: \x1b[1;32m{}\x1b[0m", fmt_kb(dr.total_kb));
            println!();

            if !dr.entries.is_empty() {
                println!("Target locations ({}):", dr.entries.len());
                for e in dr.entries {
                    println!("  {: >8}  {}", fmt_kb(e.size_kb), e.path);
                }
            }
        }
        Err(e) => {
            eprintln!("\x1b[31mError during dry-run: {}\x1b[0m", e);
        }
    }
}

fn run_clean_single(card_id: &str, cards: &[Card], auto_yes: bool) {
    let card = match cards.iter().find(|c| c.id == card_id) {
        Some(c) => c,
        None => {
            eprintln!(
                "\x1b[31mError: unknown card '{}'. Run 'alpheus scan' to list available IDs.\x1b[0m",
                card_id
            );
            return;
        }
    };

    let dr = match exec::dry_run(card) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("\x1b[31mCannot clean '{}': {}\x1b[0m", card.id, e);
            return;
        }
    };

    println!("Target: \x1b[1m{}\x1b[0m ({})", card.title, card.id);
    println!("Method: {}", dr.method);
    println!("Size:   \x1b[1;32m{}\x1b[0m", fmt_kb(dr.total_kb));
    if let Some(w) = &dr.warning {
        println!("\x1b[33mWarning: {}\x1b[0m", w);
    }

    if !auto_yes {
        print!("Proceed with cleanup? [y/N]: ");
        let _ = io::stdout().flush();
        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() || !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return;
        }
    }

    match exec::execute(card) {
        Ok(res) => {
            println!("\x1b[32m✔ {}\x1b[0m", res.message);
            let app_dir = scan::home().join(".local/share/alpheus");
            history::append(
                &app_dir,
                HistoryEntry {
                    timestamp: now_secs(),
                    card_id: card.id.clone(),
                    title: card.title.clone(),
                    freed_kb: res.freed_kb,
                    method: res.method,
                    auto: false,
                },
            );
        }
        Err(e) => {
            eprintln!("\x1b[31mExecution failed: {}\x1b[0m", e);
        }
    }
}

fn run_clean_all_safe(cards: &[Card], auto_yes: bool) {
    let safe_cards: Vec<&Card> = cards
        .iter()
        .filter(|c| c.tier == Tier::Safe && c.action != ActionKind::Explain)
        .collect();

    if safe_cards.is_empty() {
        println!("No safe-tier items to clean.");
        return;
    }

    let total_kb: u64 = safe_cards.iter().map(|c| c.size_kb).sum();
    println!(
        "Found \x1b[1m{}\x1b[0m safe categories ({}) to reclaim:",
        safe_cards.len(),
        fmt_kb(total_kb)
    );
    for c in &safe_cards {
        println!("  • {: <24}  {: >8}", c.id, fmt_kb(c.size_kb));
    }
    println!();

    if !auto_yes {
        print!("Reclaim all safe categories now? [y/N]: ");
        let _ = io::stdout().flush();
        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() || !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return;
        }
    }

    let mut total_freed = 0u64;
    let mut count = 0;
    let app_dir = scan::home().join(".local/share/alpheus");

    for card in safe_cards {
        match exec::execute(card) {
            Ok(res) => {
                total_freed += res.freed_kb;
                count += 1;
                println!("  \x1b[32m✔\x1b[0m {}", res.message);
                history::append(
                    &app_dir,
                    HistoryEntry {
                        timestamp: now_secs(),
                        card_id: card.id.clone(),
                        title: card.title.clone(),
                        freed_kb: res.freed_kb,
                        method: res.method,
                        auto: false,
                    },
                );
            }
            Err(e) => {
                eprintln!("  \x1b[31m✖ Failed to clean {}: {}\x1b[0m", card.id, e);
            }
        }
    }

    println!();
    println!(
        "\x1b[1;32mDone: Reclaimed {} across {} categories.\x1b[0m",
        fmt_kb(total_freed),
        count
    );
}

// ---------------------------------------------------------------- Top Hogs Analyzer

fn run_top(target_str: Option<&str>, limit: usize) {
    let default_path = scan::home();
    let target = target_str.map(Path::new).unwrap_or(&default_path);

    println!(
        "\x1b[1mAnalyzing largest space consumers in:\x1b[0m {}",
        target.display()
    );
    let analysis = analyze::analyze_directory(target, limit);

    if analysis.entries.is_empty() {
        println!("No measurable files or subdirectories found.");
        return;
    }

    println!("Total Scanned: \x1b[1m{}\x1b[0m\n", fmt_kb(analysis.total_scanned_kb));
    println!("  {: >8}   {: <6}  {: <30}  {}", "SIZE", "%", "NAME", "PATH");
    println!("  ────────────────────────────────────────────────────────────────────────────");

    for e in analysis.entries {
        let tag = if e.is_dir { "[dir]" } else { "[file]" };
        let bar_len = (e.percent / 5.0).round() as usize;
        let bar = "█".repeat(bar_len);

        println!(
            "  {: >8}  {: >5.1}%  {: <28}  \x1b[90m{}\x1b[0m  \x1b[36m{}\x1b[0m",
            fmt_kb(e.size_kb),
            e.percent,
            format!("{} {}", tag, e.name),
            e.path,
            bar
        );
    }
    println!();
}

// ---------------------------------------------------------------- Duplicate Scanner

fn run_dupes(target_str: Option<&str>, min_size_mb: u64) {
    let default_path = scan::home();
    let target = target_str.map(Path::new).unwrap_or(&default_path);

    println!(
        "\x1b[1mScanning for duplicate files in:\x1b[0m {} (>= {} MB)",
        target.display(),
        min_size_mb
    );
    let res = dupes::scan_duplicates(target, min_size_mb * 1024);

    if res.groups.is_empty() {
        println!(
            "\x1b[32m✔ No duplicate files found.\x1b[0m (Scanned {} files)",
            res.total_scanned_files
        );
        return;
    }

    println!(
        "Found \x1b[1m{}\x1b[0m duplicate files wasting \x1b[1;32m{}\x1b[0m across {} groups:",
        res.total_duplicate_files,
        fmt_kb(res.total_wasted_kb),
        res.groups.len()
    );
    println!("────────────────────────────────────────────────────────────────────────────");

    for (i, g) in res.groups.iter().enumerate().take(15) {
        println!(
            "  #{:<2} \x1b[1m{}\x1b[0m per file (SHA-256: {}) — Wastes \x1b[32m{}\x1b[0m",
            i + 1,
            fmt_kb(g.file_size_kb),
            g.hash_prefix,
            fmt_kb(g.wasted_kb)
        );
        println!("    ├─ \x1b[32m[Original]\x1b[0m   {}", g.original);
        for d in &g.duplicates {
            println!("    └─ \x1b[31m[Duplicate]\x1b[0m  {}", d);
        }
        println!();
    }
}

// ---------------------------------------------------------------- Growth Snapshot & Diff

fn run_snapshot(target_str: Option<&str>) {
    let default_path = scan::home();
    let target = target_str.map(Path::new).unwrap_or(&default_path);

    println!("\x1b[1mRecording disk snapshot baseline for:\x1b[0m {}", target.display());
    match snapshot::take_snapshot(target) {
        Ok(snap) => {
            println!(
                "\x1b[32m✔ Snapshot saved:\x1b[0m tracked {} directories at timestamp {}.",
                snap.entries.len(),
                snap.timestamp
            );
        }
        Err(e) => eprintln!("\x1b[31mFailed to take snapshot: {e}\x1b[0m"),
    }
}

fn run_diff(target_str: Option<&str>) {
    let default_path = scan::home();
    let target = target_str.map(Path::new).unwrap_or(&default_path);

    println!("\x1b[1mComparing disk changes for:\x1b[0m {}", target.display());
    match snapshot::diff_latest_with_live(target) {
        Ok(diff) => {
            if diff.changes.is_empty() {
                println!("\x1b[32m✔ No major disk usage changes detected since last snapshot.\x1b[0m");
                return;
            }

            println!(
                "Net Growth: \x1b[1m{}\x1b[0m ({} modified paths)",
                fmt_delta(diff.net_growth_kb),
                diff.changes.len()
            );
            println!("────────────────────────────────────────────────────────────────────────────");

            for c in diff.changes.iter().take(20) {
                let color = if c.delta_kb > 0 { "\x1b[31m" } else { "\x1b[32m" };
                println!(
                    "  {: >10}  ({: >8} → {: >8})  \x1b[90m{}\x1b[0m",
                    format!("{}{}\x1b[0m", color, fmt_delta(c.delta_kb)),
                    fmt_kb(c.old_kb),
                    fmt_kb(c.new_kb),
                    c.path
                );
            }
            println!();
        }
        Err(e) => eprintln!("\x1b[31mDiff failed: {e}\x1b[0m"),
    }
}

// ---------------------------------------------------------------- Shell Completions

fn run_completion(shell: &str) {
    match shell {
        "bash" => {
            print!(
                "{}",
                r#"_alpheus_completions() {
    local cur prev opts
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"
    opts="scan interactive status dry-run clean schedule history top dupes snapshot diff help"

    case "$prev" in
        clean|dry-run)
            local cards="cargo-target pacman-cache coredump-logs xdg-cache pkg-caches journal-logs stale-downloads py-cache yay-cache colima spotify-cache claude-vm trash"
            COMPREPLY=( $(compgen -W "${cards}" -- ${cur}) )
            return 0
            ;;
        schedule)
            COMPREPLY=( $(compgen -W "enable disable status" -- ${cur}) )
            return 0
            ;;
        completion)
            COMPREPLY=( $(compgen -W "bash zsh fish" -- ${cur}) )
            return 0
            ;;
        *)
            COMPREPLY=( $(compgen -W "${opts}" -- ${cur}) )
            return 0
            ;;
    esac
}
complete -F _alpheus_completions alpheus
"#
            );
        }
        "zsh" => {
            print!(
                "{}",
                r#"#compdef alpheus

_alpheus() {
    local -a commands
    commands=(
        'scan:Scan disk for reclaimable items'
        'interactive:Interactive keyboard cleanup menu'
        'clean:Reclaim a specific category or all safe items'
        'dry-run:Preview exact paths and bytes'
        'status:Show disk usage summary'
        'top:Analyze largest space consumers'
        'dupes:Find duplicate files'
        'snapshot:Save disk snapshot'
        'diff:Compare growth against snapshot'
        'schedule:Manage automatic background timer'
        'history:View action log'
        'completion:Generate shell completions'
    )
    _describe -t commands 'alpheus commands' commands
}

_alpheus "$@"
"#
            );
        }
        "fish" => {
            print!(
                "{}",
                r#"complete -c alpheus -f -a "scan" -d "Scan disk for reclaimable items"
complete -c alpheus -f -a "interactive" -d "Interactive keyboard cleanup menu"
complete -c alpheus -f -a "clean" -d "Reclaim a specific category or --all-safe"
complete -c alpheus -f -a "dry-run" -d "Preview exact paths and bytes"
complete -c alpheus -f -a "status" -d "Show disk usage summary"
complete -c alpheus -f -a "top" -d "Analyze largest space consumers"
complete -c alpheus -f -a "dupes" -d "Find duplicate files"
complete -c alpheus -f -a "snapshot" -d "Save disk snapshot"
complete -c alpheus -f -a "diff" -d "Compare growth against snapshot"
complete -c alpheus -f -a "schedule" -d "Manage automatic background timer"
complete -c alpheus -f -a "history" -d "View action log"
"#
            );
        }
        _ => {
            eprintln!("Unknown shell: {shell}. Supported: bash, zsh, fish");
        }
    }
}

// ---------------------------------------------------------------- Interactive TUI

fn run_interactive_tui(cards: &[Card], usage: &disk::DiskUsage) -> io::Result<()> {
    let cleanable_cards: Vec<&Card> = cards
        .iter()
        .filter(|c| c.action != ActionKind::Explain)
        .collect();

    if cleanable_cards.is_empty() {
        println!("\x1b[32m✔ No reclaimable categories found.\x1b[0m");
        return Ok(());
    }

    let mut selected: HashSet<String> = HashSet::new();
    for c in &cleanable_cards {
        if c.tier == Tier::Safe {
            selected.insert(c.id.clone());
        }
    }

    let mut cursor_idx = 0usize;
    let mut stdout = stdout();

    terminal::enable_raw_mode()?;
    execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;

    let res = (|| -> io::Result<Option<Vec<String>>> {
        loop {
            execute!(stdout, cursor::MoveTo(0, 0), Clear(ClearType::All))?;

            // Header
            let free_gb = usage.free_kb as f64 / (1024.0 * 1024.0);
            let total_gb = usage.total_kb as f64 / (1024.0 * 1024.0);
            execute!(
                stdout,
                SetForegroundColor(Color::Cyan),
                Print("  ALPHEUS STORAGE MANAGER — INTERACTIVE CLEANUP\r\n"),
                ResetColor,
                Print(format!(
                    "  Disk: {:.1} GB free / {:.1} GB total\r\n",
                    free_gb, total_gb
                )),
                Print("  ──────────────────────────────────────────────────────────────────\r\n")
            )?;

            // Card rows
            for (i, c) in cleanable_cards.iter().enumerate() {
                let is_cursor = i == cursor_idx;
                let is_checked = selected.contains(&c.id);
                let check_box = if is_checked { "[x]" } else { "[ ]" };
                let size_str = fmt_kb(c.size_kb);

                let tier_badge = match c.tier {
                    Tier::Safe => "[SAFE]",
                    Tier::WithCare => "[CARE]",
                    Tier::Manual => "[INFO]",
                };

                let tier_color = match c.tier {
                    Tier::Safe => Color::Green,
                    Tier::WithCare => Color::Yellow,
                    Tier::Manual => Color::DarkGrey,
                };

                if is_cursor {
                    execute!(
                        stdout,
                        SetBackgroundColor(Color::DarkBlue),
                        SetForegroundColor(Color::White)
                    )?;
                } else {
                    execute!(stdout, ResetColor)?;
                }

                execute!(
                    stdout,
                    Print(format!(
                        "  {} {} {: <22} {: >8}  ",
                        if is_cursor { ">" } else { " " },
                        check_box,
                        c.id,
                        size_str
                    )),
                    SetForegroundColor(tier_color),
                    Print(format!("{tier_badge}\r\n")),
                    ResetColor
                )?;
            }

            // Selected total summary
            let selected_sum: u64 = cleanable_cards
                .iter()
                .filter(|c| selected.contains(&c.id))
                .map(|c| c.size_kb)
                .sum();

            execute!(
                stdout,
                Print("  ──────────────────────────────────────────────────────────────────\r\n"),
                SetForegroundColor(Color::Green),
                Print(format!(
                    "  Selected: {} across {} categories\r\n",
                    fmt_kb(selected_sum),
                    selected.len()
                )),
                ResetColor,
                Print("\r\n")
            )?;

            // Preview box for current item
            let current = cleanable_cards[cursor_idx];
            execute!(
                stdout,
                SetForegroundColor(Color::Yellow),
                Print(format!("  Category: {}\r\n", current.title)),
                ResetColor,
                SetForegroundColor(Color::DarkGrey),
                Print(format!("  Info:     {}\r\n", current.description)),
                ResetColor
            )?;

            if let Some(cmd) = &current.command_display {
                execute!(
                    stdout,
                    SetForegroundColor(Color::Magenta),
                    Print(format!("  Command:  $ {cmd}\r\n")),
                    ResetColor
                )?;
            } else if !current.paths.is_empty() {
                execute!(
                    stdout,
                    SetForegroundColor(Color::DarkGrey),
                    Print(format!(
                        "  Paths ({}): {}\r\n",
                        current.paths.len(),
                        current.paths.join(", ")
                    )),
                    ResetColor
                )?;
            }

            // Controls footer
            execute!(
                stdout,
                cursor::MoveTo(0, 22),
                SetForegroundColor(Color::Cyan),
                Print("  [↑/k/↓/j] Navigate   [Space] Toggle   [a] All Safe   [n] None\r\n"),
                Print("  [Enter] Reclaim Selected             [q/Esc] Quit\r\n"),
                ResetColor
            )?;

            stdout.flush()?;

            // Event poll
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(None),
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            return Ok(None)
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            if cursor_idx > 0 {
                                cursor_idx -= 1;
                            } else {
                                cursor_idx = cleanable_cards.len() - 1;
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            if cursor_idx + 1 < cleanable_cards.len() {
                                cursor_idx += 1;
                            } else {
                                cursor_idx = 0;
                            }
                        }
                        KeyCode::Char(' ') => {
                            let id = cleanable_cards[cursor_idx].id.clone();
                            if selected.contains(&id) {
                                selected.remove(&id);
                            } else {
                                selected.insert(id);
                            }
                        }
                        KeyCode::Char('a') => {
                            for c in &cleanable_cards {
                                if c.tier == Tier::Safe {
                                    selected.insert(c.id.clone());
                                }
                            }
                        }
                        KeyCode::Char('n') => {
                            selected.clear();
                        }
                        KeyCode::Enter => {
                            if !selected.is_empty() {
                                return Ok(Some(selected.into_iter().collect()));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    })();

    // Restore terminal
    terminal::disable_raw_mode()?;
    execute!(stdout, terminal::LeaveAlternateScreen, cursor::Show)?;

    match res? {
        Some(to_clean) => {
            println!();
            println!("\x1b[1mExecuting cleanup for selected items ({})...\x1b[0m", to_clean.len());
            let app_dir = scan::home().join(".local/share/alpheus");
            let mut total_freed = 0u64;

            for id in to_clean {
                if let Some(card) = cards.iter().find(|c| c.id == id) {
                    match exec::execute(card) {
                        Ok(res) => {
                            total_freed += res.freed_kb;
                            println!("  \x1b[32m✔\x1b[0m {}", res.message);
                            history::append(
                                &app_dir,
                                HistoryEntry {
                                    timestamp: now_secs(),
                                    card_id: card.id.clone(),
                                    title: card.title.clone(),
                                    freed_kb: res.freed_kb,
                                    method: res.method,
                                    auto: false,
                                },
                            );
                        }
                        Err(e) => {
                            eprintln!("  \x1b[31m✖ Failed to clean {}: {}\x1b[0m", card.id, e);
                        }
                    }
                }
            }
            println!();
            println!(
                "\x1b[1;32mCleanup finished: Reclaimed {}.\x1b[0m",
                fmt_kb(total_freed)
            );
        }
        None => {
            println!("Interactive cleanup canceled.");
        }
    }

    Ok(())
}

// ---------------------------------------------------------------- Automation (systemd / launchd)

fn run_schedule(args: &[String]) {
    let sub = args.get(2).map(|s| s.as_str()).unwrap_or("status");

    #[cfg(target_os = "linux")]
    {
        let systemd_user_dir = scan::home().join(".config/systemd/user");
        let service_file = systemd_user_dir.join("alpheus-clean.service");
        let timer_file = systemd_user_dir.join("alpheus-clean.timer");

        match sub {
            "enable" | "install" => {
                let _ = fs::create_dir_all(&systemd_user_dir);
                let service_content = r#"[Unit]
Description=Alpheus automated safe storage cleanup
Documentation=https://github.com/onembyte/alpheus

[Service]
Type=oneshot
ExecStart=%h/.local/bin/alpheus clean --all-safe -y
"#;
                let timer_content = r#"[Unit]
Description=Weekly automated Alpheus safe storage cleanup

[Timer]
OnCalendar=weekly
Persistent=true

[Install]
WantedBy=timers.target
"#;
                if let Err(e) = fs::write(&service_file, service_content) {
                    eprintln!("Failed to write service file: {e}");
                    return;
                }
                if let Err(e) = fs::write(&timer_file, timer_content) {
                    eprintln!("Failed to write timer file: {e}");
                    return;
                }

                let _ = Command::new("systemctl")
                    .args(["--user", "daemon-reload"])
                    .output();
                let out = Command::new("systemctl")
                    .args(["--user", "enable", "--now", "alpheus-clean.timer"])
                    .output();

                if let Ok(o) = out {
                    if o.status.success() {
                        println!("\x1b[32m✔ Alpheus weekly background cleanup timer enabled via systemd user service.\x1b[0m");
                    } else {
                        eprintln!(
                            "Failed to enable timer: {}",
                            String::from_utf8_lossy(&o.stderr)
                        );
                    }
                }
            }
            "disable" | "uninstall" => {
                let _ = Command::new("systemctl")
                    .args(["--user", "disable", "--now", "alpheus-clean.timer"])
                    .output();
                let _ = fs::remove_file(timer_file);
                let _ = fs::remove_file(service_file);
                let _ = Command::new("systemctl")
                    .args(["--user", "daemon-reload"])
                    .output();
                println!("\x1b[32m✔ Alpheus automated cleanup timer disabled.\x1b[0m");
            }
            "status" => {
                let out = Command::new("systemctl")
                    .args(["--user", "is-active", "alpheus-clean.timer"])
                    .output();
                let active = out
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "active")
                    .unwrap_or(false);

                println!("\x1b[1mAlpheus Automation Schedule:\x1b[0m");
                if active {
                    println!("  Timer Status: \x1b[32mActive (Weekly safe cleanup enabled)\x1b[0m");
                    println!("  Service:      {}", service_file.display());
                } else {
                    println!("  Timer Status: \x1b[90mInactive / Not installed\x1b[0m");
                    println!("  Run '\x1b[1malpheus schedule enable\x1b[0m' to enable weekly automatic background cleaning.");
                }
            }
            other => {
                eprintln!("Unknown schedule command: {other}");
                println!("Usage: alpheus schedule [enable | disable | status]");
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        println!("Alpheus schedule on macOS is handled by launchd or the menu bar app.");
    }
}

fn run_history() {
    let app_dir = scan::home().join(".local/share/alpheus");
    let entries = history::load(&app_dir);
    if entries.is_empty() {
        println!("No cleanup actions recorded in history yet.");
        return;
    }

    let total: u64 = entries.iter().map(|e| e.freed_kb).sum();
    println!(
        "\x1b[1mAlpheus Action History\x1b[0m (Total Reclaimed: \x1b[1;32m{}\x1b[0m across {} actions)",
        fmt_kb(total),
        entries.len()
    );
    println!("──────────────────────────────────────────────────────────────────────");
    for e in entries.iter().rev() {
        let date_str = chrono::DateTime::from_timestamp(e.timestamp as i64, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| e.timestamp.to_string());
        println!(
            "  {}  {: <28}  {: >8}  [{}]",
            date_str,
            e.title,
            fmt_kb(e.freed_kb),
            e.method
        );
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let usage = disk::usage();

    if args.len() > 1 {
        match args[1].as_str() {
            "help" | "-h" | "--help" => {
                print_help();
                return;
            }
            "interactive" | "-i" | "tui" => {
                let cards = scan::scan_all();
                let _ = run_interactive_tui(&cards, &usage);
                return;
            }
            "top" | "analyze" => {
                let target = args.get(2).map(|s| s.as_str());
                run_top(target, 20);
                return;
            }
            "dupes" | "duplicates" => {
                let target = args.get(2).map(|s| s.as_str());
                run_dupes(target, 1); // 1 MB minimum
                return;
            }
            "snapshot" => {
                let target = args.get(2).map(|s| s.as_str());
                run_snapshot(target);
                return;
            }
            "diff" => {
                let target = args.get(2).map(|s| s.as_str());
                run_diff(target);
                return;
            }
            "completion" => {
                let sh = args.get(2).map(|s| s.as_str()).unwrap_or("bash");
                run_completion(sh);
                return;
            }
            "schedule" => {
                run_schedule(&args);
                return;
            }
            "history" => {
                run_history();
                return;
            }
            "status" => {
                if args.contains(&"--json".to_string()) {
                    let cards = scan::scan_all();
                    emit_json(&cards, &usage);
                } else {
                    let cards = scan::scan_all();
                    run_scan(&cards, &usage);
                }
                return;
            }
            "scan" => {
                let cards = scan::scan_all();
                if args.contains(&"--json".to_string()) {
                    emit_json(&cards, &usage);
                } else {
                    run_scan(&cards, &usage);
                }
                return;
            }
            "dry-run" => {
                if args.len() < 3 {
                    eprintln!("Usage: alpheus dry-run <card-id>");
                    return;
                }
                let cards = scan::scan_all();
                run_dry_run(&args[2], &cards);
                return;
            }
            "clean" => {
                let cards = scan::scan_all();
                let auto_yes = args.contains(&"-y".to_string()) || args.contains(&"--yes".to_string());
                if args.contains(&"--all-safe".to_string()) {
                    run_clean_all_safe(&cards, auto_yes);
                } else if args.len() >= 3 && !args[2].starts_with('-') {
                    run_clean_single(&args[2], &cards, auto_yes);
                } else {
                    eprintln!("Usage: alpheus clean <card-id> [-y] or alpheus clean --all-safe [-y]");
                }
                return;
            }
            other => {
                eprintln!("Unknown command: {}", other);
                print_help();
                return;
            }
        }
    }

    // Default: run scan
    let cards = scan::scan_all();
    run_scan(&cards, &usage);
}
