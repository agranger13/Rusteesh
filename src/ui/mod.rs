//! Screen rendering.
//!
//! Every function is generic over `DrawTarget<Color = Rgb565>` and so knows
//! nothing about the ST7735s or the SPI bus. They drive the real display
//! (`board::Display`) on the board and a `SimulatorDisplay` under
//! `cargo test-host`, from the very same code.

pub mod drawing_counter;
pub use drawing_counter::{draw_status, draw_paquets, refresh_background};
