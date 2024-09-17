#!/bin/bash
set -e  # Exit immediately if a command exits with a non-zero status

# Check if 'cargo' command exists
if command -v cargo &>/dev/null; then
    echo "'cargo' is already installed."
else
    echo "You need to install Rust and Cargo first."
    echo "Visit https://www.rust-lang.org/tools/install for more information."
    exit 1
fi

# Build for the current target
cargo build --release
if [[ $? -eq 0 ]]; then
    echo "Current Build successful."
else
    echo "Current Build failed."
    exit 1
fi

# Check if 'gcc' command exists
if command -v gcc &>/dev/null; then
    echo "'gcc' is already installed."
else
    echo "Installing gcc..."
    sudo apt update && sudo apt install -y build-essential
    if command -v gcc &>/dev/null; then
        echo "'gcc' installed successfully."
    else
        echo "Failed to install 'gcc'."
        exit 1
    fi
fi

# Check if 'make' command exists
if command -v make &>/dev/null; then
    echo "'make' is already installed."
else
    echo "Installing make..."
    sudo apt install -y make
    if command -v make &>/dev/null; then
        echo "'make' installed successfully."
    else
        echo "Failed to install 'make'."
        exit 1
    fi
fi

# Use Makefile to manage additional builds
make compile

# Once the build is complete, ask the user if they want to copy the executables to the 'bin' directory
read -p "Do you want to copy the executables to the 'bin' directory? [y/n]: " copy
if [[ "$copy" == "y" ]]; then
    if [[ -d bin ]]; then
        rm -rf bin/*
    else
        mkdir bin
    fi

    # Search for all executables named crypted-messages* and copy them to the 'bin' directory
    find target -type f -name "crypted-messages*" -exec cp {} bin/ \;

    echo "Executables copied to the 'bin' directory."
else
    echo "Executables not copied to the 'bin' directory."
fi
