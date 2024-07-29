# notes — docs.

## install.

1. You need to install, Rust *(rustc 1.81.0 or earlier).* If you do not have it, run the following on your terminal.
```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

2. Get the repository locally, by running the following on your terminal.
```sh
# using ssh (recommened).

git clone git@github.com:invisageable/zo.git
```

```sh
# using http.

git clone https://github.com/invisageable/zo.git
```

3. Go to the right folder and build the project, still via your terminal.

```sh
cd zo
cargo build
```

Voilà, if everything has been done without any error. You can run the project you want.

**-dev**

| crates | command               |
| ------ | --------------------- |
| zo     | `cargo run --bin zo`  |
| zom    | `cargo run --bin zom` |
| zow    | `cargo run --bin zow` |

| apps | command               |
| ---- | --------------------- |
| zoc  | `cargo run --bin zoc` |
| zoa  | `cargo run --bin zoa` |
| zop  | `cargo run --bin zop` |

*not — run also `cargo run --bin <project-name> --help` for more informations.*

**-release**

All release build are available in the `/target/release` folder at the root of the repository.