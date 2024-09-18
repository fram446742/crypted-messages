#!/bin/bash
set -e

# Check if 'cargo' command exists
if command -v cargo >/dev/null 2>&1; then
    echo "'cargo' is already installed."
else
    echo "You need to install Rust and Cargo first."
    echo "Visit https://www.rust-lang.org/tools/install for more information."
    exit 1
fi

# Check if 'cross' command exists
if command -v cross >/dev/null 2>&1; then
    echo "'cross' is already installed."
else
    echo "Installing cross using Cargo..."
    cargo install cross
    if command -v cross >/dev/null 2>&1; then
        echo "'cross' installed successfully."
    else
        echo "Failed to install 'cross'."
        exit 1
    fi
fi

# Build for the current target
cargo build --release
if [ $? -eq 0 ]; then
    echo "Current Build successful."
else
    echo "Current Build failed."
    exit 1
fi

# Check if 'gcc' command exists
if command -v gcc >/dev/null 2>&1; then
    echo "'gcc' is already installed."
else
    echo "Installing gcc using apt..."
    sudo apt update
    sudo apt install -y build-essential
    if command -v gcc >/dev/null 2>&1; then
        echo "'gcc' installed successfully."
    else
        echo "Failed to install 'gcc'."
        exit 1
    fi
fi

# Check if 'make' command exists
if command -v make >/dev/null 2>&1; then
    echo "'make' is already installed."
else
    echo "Installing make using apt..."
    sudo apt install -y make
    if command -v make >/dev/null 2>&1; then
        echo "'make' installed successfully."
    else
        echo "Failed to install 'make'."
        exit 1
    fi
fi

# Check if 'docker' command exists
if command -v docker >/dev/null 2>&1; then
    echo "'docker' is already installed."
else
    echo "You need to install Docker first."
    echo "Visit https://docs.docker.com/get-docker/ for more information."
    exit 1
fi

# Execute the Makefile
make compile

# Check if the shell is interactive
if [[ -t 0 ]]; then
    # Interactive shell, ask the user
    read -p "Do you want to copy the executables to the 'bin' directory? [y/n]: " copy
else
    # Non-interactive shell, automatically copy the executables
    copy="y"
fi

if [[ "$copy" == "y" ]]; then
    if [ -d bin ]; then
        rm -rf bin/*
    else
        mkdir bin
    fi

    # Search for all executables like crypted-messages* and copy them to the 'bin' directory
    find target -type f -name "crypted-messages*" ! -name "*.d" -exec cp {} bin/ \;

    echo "Executables copied to the 'bin' directory."
else
    echo "Executables not copied to the 'bin' directory."
fi
