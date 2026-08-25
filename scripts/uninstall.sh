#!/usr/bin/env bash
# Alpheus Clean Uninstaller for Linux & macOS
# Usage: curl -fsSL https://onembyte.github.io/alpheus/uninstall.sh | bash
set -e

BOLD="\033[1m"
GREEN="\033[32m"
CYAN="\033[36m"
YELLOW="\033[33m"
RED="\033[31m"
RESET="\033[0m"

echo -e "${BOLD}${CYAN}══════════════════════════════════════════════════════════════════════${RESET}"
echo -e "${BOLD}${CYAN}  ALPHEUS STORAGE MANAGER — UNINSTALLER${RESET}"
echo -e "${BOLD}${CYAN}══════════════════════════════════════════════════════════════════════${RESET}"

# 1. Disable and remove systemd timer
if command -v systemctl >/dev/null 2>&1; then
    systemctl --user disable --now alpheus-clean.timer 2>/dev/null || true
    rm -f "$HOME/.config/systemd/user/alpheus-clean.service" "$HOME/.config/systemd/user/alpheus-clean.timer"
    systemctl --user daemon-reload 2>/dev/null || true
fi

# 2. Remove Binary
rm -f "$HOME/.local/bin/alpheus" "/usr/local/bin/alpheus"

# 3. Remove Shell Completions
rm -f "$HOME/.local/share/bash-completion/completions/alpheus"
rm -f "$HOME/.zsh/completions/_alpheus"
rm -f "$HOME/.config/fish/completions/alpheus.fish"

# 4. Remove Omarchy Quickshell Widget & clean shell.json
rm -rf "$HOME/.config/omarchy/plugins/alpheus" "$HOME/.config/omarchy/plugins/omarchy.alpheus"

if [ -f "$HOME/.config/omarchy/shell.json" ]; then
    # Remove alpheus references from shell.json cleanly
    if command -v jq >/dev/null 2>&1; then
        TMP_JSON="$(mktemp /tmp/shell-clean.XXXXXX.json)"
        jq '
          (.bar.layout.right // []) |= map(select(.id != "alpheus" and .id != "omarchy.alpheus")) |
          (.plugins // []) |= map(select(. != "alpheus" and . != "omarchy.alpheus"))
        ' "$HOME/.config/omarchy/shell.json" > "$TMP_JSON" 2>/dev/null && mv "$TMP_JSON" "$HOME/.config/omarchy/shell.json"
    fi
fi

if command -v omarchy-shell >/dev/null 2>&1; then
    omarchy-shell shell rescanPlugins >/dev/null 2>&1 || true
fi

# 5. Remove Desktop Entry & Icons
rm -f "$HOME/.local/share/applications/alpheus.desktop"
rm -f "$HOME/.local/share/icons/hicolor/128x128/apps/alpheus.png"

# 6. Remove data directories (snapshots, history, custom rules)
rm -rf "$HOME/.local/share/alpheus"
rm -rf "$HOME/.config/alpheus"

echo -e "\n${BOLD}${GREEN}✔ Alpheus has been completely and cleanly uninstalled.${RESET}"
