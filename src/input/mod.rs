//! Reading and interpreting inputs.
//!
//! This module depends on no peripheral: it only ever sees `bool` and `u64`
//! (see [`Debouncer::update`]). It therefore builds unchanged for a PC, and its
//! tests run under `cargo test-host`.

pub mod debouncer;
pub use debouncer::{Debouncer, PressEvent, FakePin, read_low_active};
