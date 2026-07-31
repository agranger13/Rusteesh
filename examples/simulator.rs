//! Renders the real UI in a desktop window, no board required.
//!
//! Unlike the unit tests, this one is interactive: it opens an SDL2 window and
//! blocks until you close it. That is why it lives in `examples/` rather than in
//! a `#[test]` — a test must terminate on its own.
//!
//! ```text
//! cargo sim
//! ```
//!
//! It draws through the very same [`refresh_background`] and [`draw_status`]
//! used by the firmware, so what you see is what the ST7735s gets.

use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
use embedded_graphics_simulator::{OutputSettingsBuilder, SimulatorDisplay, Window};

use rusteesh::ui::{draw_status, refresh_background};

const SCREEN_W: u32 = 128;
const SCREEN_H: u32 = 160;

fn main() {
    let mut display = SimulatorDisplay::<Rgb565>::new(Size::new(SCREEN_W, SCREEN_H));

    refresh_background(&mut display);
    draw_status(&mut display, 42, 7).unwrap();

    // The real panel is tiny; scale it up so it is actually readable.
    let output_settings = OutputSettingsBuilder::new().scale(3).build();

    Window::new("Rusteesh — simulator", &output_settings).show_static(&display);
}
