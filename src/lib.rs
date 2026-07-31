//! Rusteesh firmware — ESP32-C6 with an ST7735s display.
//!
//! # Layout
//!
//! The code follows a single rule: **only one module knows about the
//! hardware**. Everything else deals in plain types (`bool`, `u64`) or in
//! abstract traits (`DrawTarget`, `InputPin`).
//!
//! | Module | Depends on `esp-hal`? | Testable on a PC? |
//! |---|---|---|
//! | `board`  | yes — that is its job | no |
//! | [`input`]  | no | yes |
//! | [`ui`]     | no | yes |
//!
//! (`board` does not appear in this documentation when it is generated without
//! the `hw` feature.)
//!
//! That split is what makes `cargo test-host` possible: with the `hw` feature
//! off, the `esp-*` crates are not compiled at all, and `input` / `ui` build
//! unchanged for x86.
//!
//! # The two dependency inversions
//!
//! Two places where the hardware would naturally have leaked into the logic,
//! and how each is resolved:
//!
//! * **Time.** [`input::Debouncer`] does not read the clock; the caller *injects*
//!   it (`update(pressed, now_ms)`). A test can therefore simulate an 800 ms
//!   press instantly. Only `main` reads real time.
//! * **The display.** [`ui`] is generic over `DrawTarget<Color = Rgb565>`, so it
//!   drives the real screen and a `SimulatorDisplay` alike.
//!
//! # Commands
//!
//! ```text
//! cargo build        # firmware for the board (`hw` feature is on by default)
//! cargo run          # build + flash + monitor through espflash
//! cargo test-host    # unit tests on the PC (alias, see .cargo/config.toml)
//! ```
//!
//! Plain `cargo test` does not work: `.cargo/config.toml` pins the RISC-V target
//! for every command, and that setting cannot be overridden per subcommand.
//! Hence the `test-host` alias.

// `no_std` for normal builds, but `std` under `cargo test`: Rust's test harness
// is built on std and cannot run without it.
#![cfg_attr(not(test), no_std)]

pub mod input;
pub mod ui;

// Board-only module.
#[cfg(feature = "hw")]
pub mod board;
