# cargo nono - Detect (possible) no_std compatibility of your crate and dependencies

## Motivation

From embedded programming, over smart contracts in Rust, to general portable crates, `#![no_std]` crates are becoming more and more widespread.
However it is currently a very cumbersome process to find out if and why (not) a crate is compatible with `no_std` usage, and often requires a lengthy trial and error process.

**cargo nono** tries to aid you in navigating the current minefield that is `no_std` usage, and it's biggest "no no"s.

## Setup

```bash
cargo install cargo-nono
```

## Usage

Run in the crate directory you want to check:

```
cargo nono check
```

The `cargo nono check` subcommand also understands the `--no-default-features` and `--features <FEATURES>` flags to help in conditional `no_std` setups.

## Features

- Tries to infer `no_std` compatibility in dependencies by looking for a `#![no_std]` attribute or the often used conditional `#![cfg_attr(not(feature = "std"), no_std)]`

### Planned features

- Warn of `[build-dependencies]` features bleeding over: [cargo#5730](https://github.com/rust-lang/cargo/issues/5730)
- Help in pinpointing which dependencies activate `std` feature flags
- Check for `use std::` statements

## License

Licensed under either of

  * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
  * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
