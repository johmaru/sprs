# Contributing

For this language development environment setup is WSL2(Ubuntu) + VSCode is recommended.

1. Install Rust and WSL2(Ubuntu).
2. ```bash
sudo apt update && sudo apt install -y lsb-release wget software-properties-common gnupg
```
3. ```bash
wget https://apt.llvm.org/llvm.sh && chmod +x llvm.sh && sudo ./llvm.sh 22 all
```
4. ```bash
sudo update-alternatives --install /usr/bin/clang clang /usr/bin/clang-22 100 && sudo update-alternatives --install /usr/bin/clang++ clang++ /usr/bin/clang++-22 100 && sudo update-alternatives --install /usr/bin/llvm-config llvm-config /usr/bin/llvm-config-22 100 && sudo update-alternatives --install /usr/bin/llvm-as llvm-as /usr/bin/llvm-as-22 100 && sudo update-alternatives --install /usr/bin/llc llc /usr/bin/llc-22 100
```
5. ```bash
sudo apt-get install zlib1g-dev libzstd-dev && sudo apt-get install libncurses5-dev libxml2-dev
```
6. Clone this repository and open it in VSCode.
7. Install the Rust extension for VSCode.
8. Build and run the project using `cargo build` and `cargo run`

## Local documentation

Build the mdBook from the repository root:

```bash
mdbook build docs
```

Serve it locally:

```bash
mdbook serve docs --hostname 127.0.0.1 --port 3000
```

Then open `http://127.0.0.1:3000`. Generated HTML is written to `docs/book/` and is not committed.
