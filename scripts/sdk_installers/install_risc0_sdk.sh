#!/bin/bash
set -e

echo "Installing Risc0 Toolchain using rzup (latest release versions)..."

# Ensure curl and bash are available
if ! command -v curl &> /dev/null; then echo "Error: curl not found." >&2; exit 1; fi
if ! command -v bash &> /dev/null; then echo "Error: bash not found." >&2; exit 1; fi

# Install rzup itself if not already present
if ! command -v rzup &> /dev/null; then
    echo "Installing rzup..."
    curl -L https://risczero.com/install | bash
    # Attempt to add rzup to PATH for the current script execution if it was just installed
    # Common install location for rzup is $HOME/.cargo/bin
    if [[ ":$PATH:" != *":$HOME/.cargo/bin:"* ]]; then 
        export PATH="$HOME/.cargo/bin:$PATH"
    fi 
    if ! command -v rzup &> /dev/null; then 
        echo "Error: rzup still not found after install attempt. Ensure $HOME/.cargo/bin is in your PATH or rzup installed correctly." >&2; 
        exit 1;
    fi
else
    echo "rzup already installed."
fi

# Install the latest released Risc0 toolchain
rzup install

# Verify Risc0 installation
echo "Verifying Risc0 installation..."
if ! command -v cargo &> /dev/null; then echo "Error: cargo not found." >&2; exit 1; fi
cargo risczero --version || (echo "Error: cargo risczero command failed!" >&2 && exit 1)

echo "Risc0 Toolchain installation (latest release) successful." 