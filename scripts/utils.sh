#!/bin/bash
# Common utility functions for shell scripts

# Checks if a tool is installed and available in PATH.
# Usage: is_tool_installed <tool_name>
# Returns 0 if found, 1 otherwise.
is_tool_installed() {
    command -v "$1" &> /dev/null
}

# Ensures a tool is installed. Exits with an error if not.
# Usage: ensure_tool_installed <tool_name> [optional_purpose_message]
# Example: ensure_tool_installed curl "to download files"
ensure_tool_installed() {
    local tool_name="$1"
    local purpose_message="$2"

    if ! is_tool_installed "${tool_name}"; then
        echo "Error: Required tool '${tool_name}' could not be found." >&2
        if [ -n "${purpose_message}" ]; then
            echo "       It is needed ${purpose_message}." >&2
        fi
        echo "       Please install it first and ensure it is in your PATH." >&2
        exit 1
    fi
} 