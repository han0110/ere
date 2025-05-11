#!/bin/bash
set -e

echo "Installing Risc0 Toolchain using rzup (latest release versions)..."

# Get the directory of the currently executing script to reliably source utils.sh
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && pwd)"
if [ -f "${SCRIPT_DIR}/../utils.sh" ]; then
    # shellcheck source=../utils.sh
    source "${SCRIPT_DIR}/../utils.sh"
else
    echo "Error: utils.sh not found. Expected at ${SCRIPT_DIR}/../utils.sh" >&2
    exit 1
fi

ensure_tool_installed "curl" "to download the rzup installer"
ensure_tool_installed "bash" "as the rzup installer script uses bash"

# Install rzup itself if not already present
if ! is_tool_installed "rzup"; then
    echo "Attempting to install rzup..."
    # The rzup installer (risczero.com/install) installs rzup to $HOME/.risc0/bin
    # and should modify shell profiles like .bashrc to add it to PATH.
    curl -L https://risczero.com/install | bash

    # For the current script's execution, we need to add the rzup path explicitly
    # as the .bashrc changes won't affect this running script instance.
    RZUP_BIN_DIR="${HOME}/.risc0/bin"
    if [ -d "${RZUP_BIN_DIR}" ] && [[ ":$PATH:" != *":${RZUP_BIN_DIR}:"* ]]; then
        echo "Adding ${RZUP_BIN_DIR} to PATH for current script session."
        export PATH="${RZUP_BIN_DIR}:$PATH"
    fi

    # Re-check if rzup is now in PATH
    if ! is_tool_installed "rzup"; then
        echo "Error: rzup command not found after installation attempt." >&2
        echo "       Please check if ${RZUP_BIN_DIR} was created and if it's in your PATH for new shells." >&2
        echo "       You might need to source your ~/.bashrc or similar shell profile." >&2
        exit 1
    fi
    echo "rzup installed successfully and added to PATH for this session."
else
    echo "rzup already installed and in PATH."
fi

# Now that rzup is confirmed to be in PATH for this script, install the Risc0 toolchain
echo "Running 'rzup install' to install/update Risc0 toolchain..."
rzup install

# Verify Risc0 installation
echo "Verifying Risc0 installation..."
ensure_tool_installed "cargo" "as cargo-risczero needs it"
cargo risczero --version || (echo "Error: cargo risczero command failed!" >&2 && exit 1)

echo "Risc0 Toolchain installation (latest release) successful."
echo "The rzup installer might have updated your shell configuration files (e.g., ~/.bashrc, ~/.zshrc)."
echo "To ensure rzup and Risc0 tools are available in your current shell session if this was a new installation,"
echo "you may need to source your shell profile (e.g., 'source ~/.bashrc') or open a new terminal." 