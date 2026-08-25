#!/usr/bin/env bash
# Alpheus Universal One-Line Installer for Linux & macOS
# Usage: curl -fsSL https://onembyte.github.io/alpheus/install.sh | bash
set -e

BOLD="\033[1m"
GREEN="\033[32m"
CYAN="\033[36m"
YELLOW="\033[33m"
RED="\033[31m"
RESET="\033[0m"

REPO="onembyte/alpheus"
GITHUB_RAW="https://raw.githubusercontent.com/${REPO}/main"
GITHUB_RELEASES="https://github.com/${REPO}/releases"

echo -e "${BOLD}${CYAN}══════════════════════════════════════════════════════════════════════${RESET}"
echo -e "${BOLD}${CYAN}  ALPHEUS STORAGE MANAGER — INSTALLER${RESET}"
echo -e "${BOLD}${CYAN}══════════════════════════════════════════════════════════════════════${RESET}"

# 1. Detect OS & Architecture
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH_RAW="$(uname -m)"

case "$ARCH_RAW" in
    x86_64|amd64)
        ARCH="x86_64"
        ;;
    aarch64|arm64)
        ARCH="aarch64"
        ;;
    *)
        ARCH="$ARCH_RAW"
        ;;
esac

echo -e "  Detected Platform: ${BOLD}${OS} (${ARCH})${RESET}"

# 2. Determine installation directory
if [ "$OS" = "darwin" ] && [ -w "/usr/local/bin" ]; then
    INSTALL_DIR="/usr/local/bin"
elif [ -d "$HOME/.local/bin" ] || [ "$OS" = "linux" ]; then
    INSTALL_DIR="$HOME/.local/bin"
else
    INSTALL_DIR="$HOME/.local/bin"
fi
mkdir -p "$INSTALL_DIR"

TMP_DIR="$(mktemp -d /tmp/alpheus-install.XXXXXX)"
cleanup() {
    rm -rf "$TMP_DIR"
}
trap cleanup EXIT

# 3. Download prebuilt binary or compile from source
INSTALLED=0

echo -e "  Fetching Alpheus..."
RELEASE_URL="${GITHUB_RELEASES}/latest/download/alpheus-${OS}-${ARCH}.tar.gz"

if curl -fsSLI "$RELEASE_URL" >/dev/null 2>&1; then
    echo -e "  Downloading prebuilt release from GitHub..."
    if curl -fsSL "$RELEASE_URL" -o "$TMP_DIR/alpheus.tar.gz"; then
        tar -xzf "$TMP_DIR/alpheus.tar.gz" -C "$TMP_DIR"
        # Find binary in extracted archive
        BIN_PATH="$(find "$TMP_DIR" -type f -name "alpheus" | head -n 1)"
        if [ -n "$BIN_PATH" ]; then
            install -m755 "$BIN_PATH" "$INSTALL_DIR/alpheus"
            INSTALLED=1
        fi
    fi
fi

# Fallback: build from source if prebuilt release not available or failed
if [ "$INSTALLED" -eq 0 ]; then
    # Check if local repo is present
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}" 2>/dev/null)" 2>/dev/null && pwd || echo "")"
    LOCAL_CARGO="$SCRIPT_DIR/../src-tauri/Cargo.toml"

    if [ -f "$LOCAL_CARGO" ]; then
        echo -e "  Compiling from local source..."
        (cd "$SCRIPT_DIR/../src-tauri" && cargo build --release --bin alpheus)
        install -m755 "$SCRIPT_DIR/../src-tauri/target/release/alpheus" "$INSTALL_DIR/alpheus"
        INSTALLED=1
    elif command -v cargo >/dev/null 2>&1 && command -v git >/dev/null 2>&1; then
        echo -e "  Cloning and compiling latest source via cargo..."
        git clone --depth 1 "https://github.com/${REPO}.git" "$TMP_DIR/alpheus-src"
        (cd "$TMP_DIR/alpheus-src/src-tauri" && cargo build --release --bin alpheus)
        install -m755 "$TMP_DIR/alpheus-src/src-tauri/target/release/alpheus" "$INSTALL_DIR/alpheus"
        INSTALLED=1
    else
        echo -e "${RED}Error: Precompiled binary not found and Rust/Cargo is not installed.${RESET}"
        echo -e "Please install Rust from https://rustup.rs or download a release from ${GITHUB_RELEASES}"
        exit 1
    fi
fi

echo -e "  ${GREEN}✔ Installed binary:${RESET} ${INSTALL_DIR}/alpheus"

# 4. Configure Shell Auto-Completions
echo -e "  Configuring shell completions..."
if [ -d "$HOME/.local/share/bash-completion/completions" ] || [ -f "$HOME/.bashrc" ]; then
    mkdir -p "$HOME/.local/share/bash-completion/completions"
    "$INSTALL_DIR/alpheus" completion bash > "$HOME/.local/share/bash-completion/completions/alpheus" 2>/dev/null || true
fi

if [ -d "$HOME/.zsh" ] || [ -f "$HOME/.zshrc" ]; then
    mkdir -p "$HOME/.zsh/completions"
    "$INSTALL_DIR/alpheus" completion zsh > "$HOME/.zsh/completions/_alpheus" 2>/dev/null || true
fi

if [ -d "$HOME/.config/fish" ]; then
    mkdir -p "$HOME/.config/fish/completions"
    "$INSTALL_DIR/alpheus" completion fish > "$HOME/.config/fish/completions/alpheus.fish" 2>/dev/null || true
fi

# 5. Omarchy OS Integration
if [ -d "/usr/share/omarchy/shell" ] || [ -f "/etc/omarchy/version" ] || [ -d "$HOME/.config/omarchy" ]; then
    echo -e "  Detected ${BOLD}Omarchy OS${RESET} environment."
    PLUGIN_DIR="$HOME/.config/omarchy/plugins/alpheus"
    mkdir -p "$PLUGIN_DIR"

    # Try local plugin folder first, or fetch from GitHub
    if [ -d "$SCRIPT_DIR/../plugins/alpheus" ]; then
        cp -r "$SCRIPT_DIR/../plugins/alpheus/"* "$PLUGIN_DIR/"
    else
        curl -fsSL "${GITHUB_RAW}/plugins/alpheus/manifest.json" -o "$PLUGIN_DIR/manifest.json" 2>/dev/null || true
        curl -fsSL "${GITHUB_RAW}/plugins/alpheus/Panel.qml" -o "$PLUGIN_DIR/Panel.qml" 2>/dev/null || true
    fi

    # Trigger hot-reload in Omarchy shell
    if command -v omarchy-shell >/dev/null 2>&1; then
        omarchy-shell shell rescanPlugins >/dev/null 2>&1 || true
    fi
    echo -e "  ${GREEN}✔ Installed Omarchy Quickshell top-bar widget to:${RESET} $PLUGIN_DIR"
fi

# 6. Desktop Entry & App Icon (Linux)
if [ "$OS" = "linux" ]; then
    APPS_DIR="$HOME/.local/share/applications"
    ICONS_DIR="$HOME/.local/share/icons/hicolor/128x128/apps"
    mkdir -p "$APPS_DIR" "$ICONS_DIR"

    if [ -f "$SCRIPT_DIR/../alpheus.desktop" ]; then
        cp "$SCRIPT_DIR/../alpheus.desktop" "$APPS_DIR/alpheus.desktop"
    else
        curl -fsSL "${GITHUB_RAW}/alpheus.desktop" -o "$APPS_DIR/alpheus.desktop" 2>/dev/null || true
    fi

    if [ -f "$SCRIPT_DIR/../src-tauri/icons/128x128.png" ]; then
        cp "$SCRIPT_DIR/../src-tauri/icons/128x128.png" "$ICONS_DIR/alpheus.png"
    else
        curl -fsSL "${GITHUB_RAW}/src-tauri/icons/128x128.png" -o "$ICONS_DIR/alpheus.png" 2>/dev/null || true
    fi
    echo -e "  ${GREEN}✔ Installed desktop menu entry & application icon.${RESET}"
fi

# 7. Check PATH
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo -e "\n  ${YELLOW}Notice:${RESET} $INSTALL_DIR is not currently in your PATH."
    echo -e "  Add it to your shell configuration (e.g. ~/.bashrc or ~/.zshrc):"
    echo -e "    ${BOLD}export PATH=\"\$HOME/.local/bin:\$PATH\"${RESET}"
fi

echo -e "\n${BOLD}${GREEN}✔ Alpheus installed successfully!${RESET}"
echo -e "Run '${BOLD}alpheus${RESET}' to scan your drive, or '${BOLD}alpheus -i${RESET}' for interactive cleanup."
