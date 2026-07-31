//! Screen rendering.
//!
//! Every function is generic over `DrawTarget<Color = Rgb565>` and so knows
//! nothing about the ST7735s or the SPI bus. They accept the real display
//! (`board::Display`) and the [`FakeDrawTarget`] stub interchangeably.

pub mod drawing_counter;
pub use drawing_counter::{draw_status, refresh_background, FakeDrawTarget};
