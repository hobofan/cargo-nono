## cargo nono - Detect (possible) no_std compatibility of your crate and dependencies

## Motivation

From embedded programming, over smart contracts in Rust, to general cross-platform portable crates, `#![no_std]` crates are becoming more and more widespread.
However it is currently a very cumbersome process to find out if and why (not) a crate is compatible with `no_std` usage, and often requires a lengthy trial and error process, and digging through the source of all your dependencies.

**cargo nono** tries to aid you in navigating the current minefield that is `no_std` usage, and it's biggest "no no"s.

## Setup

```bash
cargo install cargo-nono
# For warnings with more informative messages install like this
RUSTFLAGS="--cfg procmacro2_semver_exempt" cargo install cargo-nono
```

## Demo

[![asciicast](https://asciinema.org/a/212278.svg)](https://asciinema.org/a/212278)

## Usage

Run in the crate directory you want to check:

```
cargo nono check
```

The `cargo nono check` subcommand also understands the `--no-default-features` and `--features <FEATURES>` flags to help in conditional `no_std` setups.

## Features

- Tries to infer `no_std` compatibility in dependencies by looking for a `#![no_std]` attribute or the often used conditional `#![cfg_attr(not(feature = "std"), no_std)]`
- Helps in pinpointing which dependencies and feature flags activate `std` feature flags

### Planned features

- Warn of `[build-dependencies]` features bleeding over: [cargo#5730](https://github.com/rust-lang/cargo/issues/5730)

## License

Licensed under either of

  * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
  * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
