# Crypted Messages

Welcome to Crypted Messages! This repository contains a basic implementation of a chat messaging app using the console.

[![Rust](https://github.com/fram446742/crypted-messages/actions/workflows/rust.yml/badge.svg)](https://github.com/fram446742/crypted-messages/actions/workflows/rust.yml)[![Rust Windows-Linux-Darwin Cross Build](https://github.com/fram446742/crypted-messages/actions/workflows/main.yml/badge.svg)](https://github.com/fram446742/crypted-messages/actions/workflows/main.yml)

## Features

- **Uses Encryption**: Uses Aes256 encryption for the messages.
- **User-Friendly Interface**: Easy-to-use command-line interface.
- **Secure**: Implements best practices for cryptographic security.

## Getting Started

### Prerequisites

- Rust
- Chocolatey (if running the batch script)

### Installation

1. Clone the repository:

    ```sh

    git clone https://github.com/fram446742/crypted-messages.git
    
    ```

2. Navigate to the project directory:

    ```sh
    cd crypted-messages
    ```

3. Build the proyect:

    ```sh
    cargo build --release
    ```

## Usage

### Execute the program

```sh
cargo run
```

## Important Notes

You will need to have Docker with an specific image to be able to cross-compile the program using [build-all.bat](build-all.bat).

## Contributing

We welcome contributions! Please read our [Contributing Guidelines](CONTRIBUTING.md) for more details.

## License

This project is licensed under the GNU License - see the [LICENSE](LICENSE) file for details.

## Contact

For any questions or suggestions, feel free to open an issue or contact us at [francisco446742@gmail.com](mailto:francisco446472@gmail.com).

---

Happy Chatting! 🚀
