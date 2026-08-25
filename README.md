<p align="center">
  <img src="src-tauri/icons/128x128%402x.png" width="96" alt="Alpheus icon" />
</p>

<h1 align="center">Alpheus</h1>

<p align="center">
  The storage manager and cleanup engine for macOS and Linux (Omarchy / Arch). Honest, drillable, and safe.
  <br />
  <a href="#-quick-install">Quick Install</a> ·
  <a href="#why">Why</a> ·
  <a href="#features">Features</a> ·
  <a href="#cli-reference">CLI Reference</a> ·
  <a href="#installation-by-platform">Manual Build</a> ·
  <a href="#omarchy-top-bar-widget">Omarchy Widget</a> ·
  <a href="#safety-model">Safety Model</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20(Omarchy%20%2F%20Arch)-blue" alt="Platforms" />
  <img src="https://img.shields.io/badge/UI-Tauri%20GUI%20%2B%20Terminal%20TUI%20%2B%20Quickshell-teal" alt="Interfaces" />
  <img src="https://img.shields.io/badge/license-MIT-green" alt="MIT License" />
</p>

---

## ⚡ Quick Install

Install the Alpheus CLI, interactive TUI, shell completions, and Omarchy desktop widget with a single command on **Linux (Omarchy / Arch)** and **macOS**:

```bash
curl -fsSL https://onembyte.github.io/alpheus/install.sh | bash
```

> *(Fallback via raw GitHub)*:
> ```bash
> curl -fsSL https://raw.githubusercontent.com/onembyte/alpheus/main/scripts/install.sh | bash
> ```

---

## Why

Every developer workstation slowly fills up with gigabytes of dead weight: forgotten Docker VM disks, dozens of `node_modules`, compiled Cargo `target/` directories, Pacman package caches, Xcode DerivedData, stale coredumps, and browser caches.

Finding and cleaning them usually means running messy, unsafe `du` and `find -delete` commands. **Alpheus** turns that into a unified, safe, one-glance tool with three interfaces:
1. **Interactive Terminal CLI & TUI (`alpheus`)** for keyboard-driven terminal workflows.
2. **Omarchy Top Bar Quickshell Widget** for instant live stats and one-click cleanup on Arch / Omarchy OS.
3. **Liquid Glass Desktop GUI (`alpheus-app`)** on macOS and Linux desktop environments.

---

## Features

- **Deep Developer Scanners** (`du -sk` exact byte measurements, parallelized in Rust):
  - **Rust Projects**: Cargo `target/` build directories.
  - **JavaScript / Web**: `node_modules` (verified against git status before offering one-click deletion) and `.next` build outputs.
  - **Python**: `__pycache__`, `.pytest_cache`, `.ruff_cache`, `.mypy_cache`, and pip/uv/poetry caches.
  - **Package Managers**: Pacman cache (`/var/cache/pacman/pkg`), Yay / AUR build cache (`~/.cache/yay`), npm, pnpm, NuGet, Go modules, Playwright browsers, Homebrew.
  - **Containers & Runtimes**: Docker & Podman (`docker system prune`), Colima VM disks, Flatpak unused runtimes (`flatpak uninstall --unused`).
  - **System Logs & Coredumps**: Systemd journal logs (`journalctl --vacuum-size`), crash coredumps (`/var/lib/systemd/coredump`).
  - **App Scratch & Caches**: `~/.cache` (Linux) and `~/Library/Caches` (macOS), Spotify cache, VS Code cached extensions.
  - **Stale Downloads**: Unaccessed large files in `~/Downloads` untouched for >30 days.
  - **macOS Specific**: Xcode DerivedData, iOS DeviceSupport symbols, iOS Simulators, Time Machine APFS snapshots.
  - **Trash**: FreeDesktop Trash (`~/.local/share/Trash`) and macOS Trash (`~/.Trash`).

- **Proven Safe Before Deleting**:
  - `node_modules` are only marked safe when the git repo is *clean, has a remote, and has zero unpushed commits* — the card displays the git proof.
  - Hard denylist prevents touching anything outside `$HOME` (except allowlisted system caches).
  - Dry run preview is re-verified before any file removal.

- **Automated Background Maintenance**:
  - Built-in `alpheus schedule` sets up a systemd user timer on Linux for background safe cleanup.

---

## CLI Reference

### `alpheus scan`
Scans the filesystem and displays a color-coded table of reclaimable items categorized by safety tier:
```bash
alpheus scan
```

### `alpheus -i` (Interactive TUI)
Launches the full interactive terminal menu with keyboard navigation:
- `↑` / `k`, `↓` / `j`: Move selection
- `[Space]`: Toggle category selection
- `[a]`: Select all safe categories
- `[n]`: Clear all selections
- `[Enter]`: Execute cleanup for all selected items
- `[q]` / `[Esc]`: Exit without cleaning

```bash
alpheus -i
```

### `alpheus browse [dir]` (Directory Tree Explorer)
Interactive ncdu-style folder tree navigator to drill into subdirectories, inspect space hogs, and trash unwanted items:
```bash
alpheus browse ~
```

### `alpheus top [dir]` (Space Hogs)
Finds the largest space consumers in any folder with visual proportional bars:
```bash
alpheus top ~
```

### `alpheus dupes [dir]` (Duplicate Finder)
Fast multi-stage SHA-256 duplicate file detector:
```bash
alpheus dupes ~/Downloads
```

### `alpheus snapshot [dir]` & `alpheus diff [dir]` (Growth Tracker)
Takes a baseline size snapshot and tracks what grew or shrank on disk over time:
```bash
alpheus snapshot ~
alpheus diff ~
```

### `alpheus clean --all-safe [-y]`
Reclaims all `safe`-tier categories in one go:
```bash
alpheus clean --all-safe -y
```

### `alpheus schedule [enable | disable | status]`
Configures systemd user background timer to automatically clean safe caches:
```bash
alpheus schedule enable
alpheus schedule status
```

### `alpheus update`
Updates the Alpheus binary in-place to the latest release:
```bash
alpheus update
```

---

## Installation by Platform

### Linux (Omarchy OS / Arch Linux)

#### 1. Quick One-Liner (Recommended):

```bash
curl -fsSL https://onembyte.github.io/alpheus/install.sh | bash
```

#### 2. From Source / Cargo:

```bash
# Clone repository
git clone https://github.com/onembyte/alpheus.git
cd alpheus/src-tauri

# Build optimized release binary and install to ~/.local/bin
cargo build --release --bin alpheus
install -m755 target/release/alpheus ~/.local/bin/alpheus

# Verify installation
alpheus scan
```

#### 3. Omarchy Top Bar Quickshell Widget:

The installer deploys the widget automatically. To configure manually:

```bash
mkdir -p ~/.config/omarchy/plugins/omarchy.alpheus
cp -r plugins/omarchy.alpheus/* ~/.config/omarchy/plugins/omarchy.alpheus/
```

Add `"omarchy.alpheus"` to `~/.config/omarchy/shell.json` in your plugins and bar layout:

```json
{
  "bar": {
    "layout": {
      "right": [
        { "id": "omarchy.power" },
        { "id": "omarchy.alpheus" }
      ]
    }
  },
  "plugins": [
    "omarchy.alpheus"
  ]
}
```

---

### macOS

#### 1. Quick CLI Install:

```bash
curl -fsSL https://onembyte.github.io/alpheus/install.sh | bash
```

#### 2. Desktop App:

Download the latest `Alpheus-macOS.dmg`, drag **Alpheus** to Applications, then remove the quarantine attribute:

```bash
xattr -d com.apple.quarantine /Applications/Alpheus.app
```

---

## Safety Model

| Rule | Enforcement |
|---|---|
| **Three tiers** | `safe` (green, regenerable) · `with-care` (yellow, confirm dialog) · `manual` (grey, informational) |
| **Dry run before delete** | All targets are re-measured and listed before execution |
| **Denylist** | Refuses deletion of anything outside `$HOME` except allowlisted system caches (`/var/cache/pacman/pkg`, `/var/lib/systemd/coredump`); untouchables like `~/.ssh`, `~/.claude`, `~/Documents/prod` are hard-blocked |
| **Trash threshold** | Deletions under 5 GB go to FreeDesktop / macOS Trash; direct `rm` is only permitted for allowlisted regenerable targets |
| **Allowlisted commands** | Command cards (`paccache`, `journalctl`, `coredumpctl`, `docker system prune`) run fixed argvs with no shell injection |

---

## Project Layout

```
├── src/                    # React + TypeScript desktop GUI
├── src-tauri/
│   ├── src/bin/alpheus.rs  # Standalone CLI & Interactive TUI binary
│   ├── src/scan.rs         # Parallel multi-platform scanners & git proofs
│   ├── src/exec.rs         # Safety rules & execution engine
│   ├── src/browse.rs       # Interactive directory tree explorer
│   ├── src/analyze.rs      # Directory size analyzer & space hogs
│   ├── src/dupes.rs        # SHA-256 duplicate file detector
│   ├── src/snapshot.rs     # Snapshot baseline & growth diff engine
│   ├── src/rules.rs        # User custom rules (~/.config/alpheus/rules.json)
│   ├── src/disk.rs         # POSIX disk usage parser
│   ├── src/history.rs      # JSON action log
│   ├── src/google.rs       # Google Drive BYOK integration
│   └── src/lib.rs          # Core library & Tauri bindings
├── plugins/omarchy.alpheus/ # Omarchy Quickshell Widget
│   ├── manifest.json       # Omarchy plugin manifest
│   ├── BarWidget.qml       # Top bar disk indicator
│   └── Panel.qml           # Popout dropdown panel
└── docs/
    ├── index.html          # GitHub Pages landing page
    └── install.sh          # Hosted installer endpoint
```

---

## License

[MIT](LICENSE)
