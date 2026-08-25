//! Interactive terminal directory drill-down explorer (ncdu-style).

use crate::analyze;
use crate::scan::{self, is_denied};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{self, Clear, ClearType},
};
use std::io::{self, stdout, Write};
use std::path::{Path, PathBuf};

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

pub fn run_interactive_browser(start_dir: &Path) -> io::Result<()> {
    let mut current_dir = if start_dir.is_dir() {
        start_dir.to_path_buf()
    } else {
        scan::home()
    };

    let mut cursor_idx = 0usize;
    let mut stdout = stdout();

    terminal::enable_raw_mode()?;
    execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;

    let res = (|| -> io::Result<()> {
        loop {
            let analysis = analyze::analyze_directory(&current_dir, 50);
            if cursor_idx >= analysis.entries.len() && !analysis.entries.is_empty() {
                cursor_idx = analysis.entries.len() - 1;
            }

            execute!(stdout, cursor::MoveTo(0, 0), Clear(ClearType::All))?;

            // Top Header
            execute!(
                stdout,
                SetForegroundColor(Color::Cyan),
                Print("  ALPHEUS DIRECTORY EXPLORER\r\n"),
                ResetColor,
                SetForegroundColor(Color::Yellow),
                Print(format!("  Location: {}\r\n", current_dir.display())),
                ResetColor,
                Print(format!(
                    "  Total: {} ({} items)\r\n",
                    fmt_kb(analysis.total_scanned_kb),
                    analysis.entries.len()
                )),
                Print("  ────────────────────────────────────────────────────────────────────────────\r\n")
            )?;

            if analysis.entries.is_empty() {
                execute!(
                    stdout,
                    SetForegroundColor(Color::DarkGrey),
                    Print("  (empty directory or inaccessible)\r\n"),
                    ResetColor
                )?;
            } else {
                let max_display = 18;
                let start_idx = if cursor_idx >= max_display {
                    cursor_idx - max_display + 1
                } else {
                    0
                };
                let end_idx = (start_idx + max_display).min(analysis.entries.len());

                for i in start_idx..end_idx {
                    let e = &analysis.entries[i];
                    let is_cursor = i == cursor_idx;
                    let tag = if e.is_dir { "[dir]" } else { "[file]" };
                    let bar_len = (e.percent / 5.0).round() as usize;
                    let bar = "█".repeat(bar_len);

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
                            "  {} {: >8} {: >5.1}%  {: <6} {: <30} ",
                            if is_cursor { ">" } else { " " },
                            fmt_kb(e.size_kb),
                            e.percent,
                            tag,
                            if e.name.len() > 30 {
                                format!("{}...", &e.name[..27])
                            } else {
                                e.name.clone()
                            }
                        )),
                        SetForegroundColor(Color::Cyan),
                        Print(format!("{bar}\r\n")),
                        ResetColor
                    )?;
                }
            }

            // Footer navigation bar
            execute!(
                stdout,
                cursor::MoveTo(0, 24),
                Print("  ────────────────────────────────────────────────────────────────────────────\r\n"),
                SetForegroundColor(Color::Cyan),
                Print("  [↑/k/↓/j] Navigate   [Enter/l] Open   [Backspace/h] Parent Directory\r\n"),
                Print("  [d] Move to Trash    [q/Esc] Exit\r\n"),
                ResetColor
            )?;

            stdout.flush()?;

            // Keyboard input handling
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            return Ok(())
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            if cursor_idx > 0 {
                                cursor_idx -= 1;
                            } else if !analysis.entries.is_empty() {
                                cursor_idx = analysis.entries.len() - 1;
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            if cursor_idx + 1 < analysis.entries.len() {
                                cursor_idx += 1;
                            } else {
                                cursor_idx = 0;
                            }
                        }
                        KeyCode::Enter | KeyCode::Char('l') => {
                            if !analysis.entries.is_empty() {
                                let selected = &analysis.entries[cursor_idx];
                                if selected.is_dir {
                                    let next_path = PathBuf::from(&selected.path);
                                    if !is_denied(&next_path) && next_path.is_dir() {
                                        current_dir = next_path;
                                        cursor_idx = 0;
                                    }
                                }
                            }
                        }
                        KeyCode::Backspace | KeyCode::Char('h') => {
                            if let Some(parent) = current_dir.parent() {
                                if !is_denied(parent) {
                                    current_dir = parent.to_path_buf();
                                    cursor_idx = 0;
                                }
                            }
                        }
                        KeyCode::Char('d') => {
                            if !analysis.entries.is_empty() {
                                let target_path = PathBuf::from(&analysis.entries[cursor_idx].path);
                                if !is_denied(&target_path) {
                                    let _ = trash::delete(&target_path);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    })();

    terminal::disable_raw_mode()?;
    execute!(stdout, terminal::LeaveAlternateScreen, cursor::Show)?;

    res
}
