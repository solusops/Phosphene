#!/bin/bash

set -e

echo "======================================"
echo "    Phosphene Installation Script     "
echo "======================================"

echo "[1/4] Compiling Phosphene in release mode..."
cargo build --release

echo "[2/4] Installing binary to ~/.local/bin..."
mkdir -p ~/.local/bin
cp target/release/phosphene ~/.local/bin/

if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
    echo "WARNING: ~/.local/bin is not in your PATH. You may need to add it to your ~/.bashrc or ~/.zshrc."
fi

echo "[3/4] Generating default config..."
CONFIG_DIR="$HOME/.config/phosphene"
CONFIG_FILE="$CONFIG_DIR/config.toml"
mkdir -p "$CONFIG_DIR"
if [ ! -f "$CONFIG_FILE" ]; then
    cat << 'EOF' > "$CONFIG_FILE"
# Phosphene Configuration
max_resolution = 1024
color_theme = "viridis"
window_spawn_behavior = "center"
# cache_dir = "~/.cache/phosphene" # Uncomment to set a custom cache path
EOF
    echo "  -> Created $CONFIG_FILE"
else
    echo "  -> Config already exists at $CONFIG_FILE"
fi

echo "[4/4] Installing File Manager Integrations..."

# Nautilus Integration
NAUTILUS_EXT_DIR="$HOME/.local/share/nautilus-python/extensions"
if [ -d "$HOME/.local/share/nautilus-python" ] || command -v nautilus &> /dev/null; then
    echo "  -> Nautilus detected. Installing Python extension..."
    mkdir -p "$NAUTILUS_EXT_DIR"
    cp nautilus/phosphene.py "$NAUTILUS_EXT_DIR/"
    echo "  -> Note: You may need to restart Nautilus (nautilus -q)."
fi

# KDE Dolphin Integration
KDE_SVC_DIR_1="$HOME/.local/share/kio/servicemenus"
KDE_SVC_DIR_2="$HOME/.local/share/kservices5/ServiceMenus"
if command -v dolphin &> /dev/null || [ -d "$KDE_SVC_DIR_1" ] || [ -d "$KDE_SVC_DIR_2" ]; then
    echo "  -> KDE Dolphin detected. Installing Desktop Service..."
    if [ -d "$KDE_SVC_DIR_1" ] || [ ! -d "$KDE_SVC_DIR_2" ]; then
        mkdir -p "$KDE_SVC_DIR_1"
        cp kde/phosphene.desktop "$KDE_SVC_DIR_1/"
    else
        mkdir -p "$KDE_SVC_DIR_2"
        cp kde/phosphene.desktop "$KDE_SVC_DIR_2/"
    fi
fi

# Cinnamon Nemo Integration
NEMO_ACT_DIR="$HOME/.local/share/nemo/actions"
if [ -d "$HOME/.local/share/nemo" ] || command -v nemo &> /dev/null; then
    echo "  -> Nemo detected. Installing Nemo Action..."
    mkdir -p "$NEMO_ACT_DIR"
    cp nemo/phosphene.nemo_action "$NEMO_ACT_DIR/"
fi

echo "======================================"
echo "      Installation Complete!          "
echo "======================================"
