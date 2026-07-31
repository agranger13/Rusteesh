//! Rendering of the button counters.
//!
//! Nothing here knows about the ST7735s: everything goes through `DrawTarget`,
//! which is what makes these functions testable on a PC with
//! [`FakeDrawTarget`].

use core::fmt::Write;

use embedded_graphics::{
    mono_font::{ascii::FONT_9X15, MonoTextStyle},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{PrimitiveStyleBuilder, Rectangle, Line},
    text::{Alignment, Text},
};

/// Draws both counters, `A` to the left and `B` to the right of the centre
/// line, formatted as `A:<n>`.
///
/// Erases nothing: the caller is expected to have cleared the area first (see
/// `main`, which fills a black rectangle before every update), otherwise the
/// digits pile up on top of each other.
///
/// Individual draw errors are swallowed — on a display, a lost pixel is no
/// reason to bring the firmware down. The `Result` is therefore always `Ok`; it
/// is kept only so the function stays composable.
///
/// # Panics
///
/// If formatting a counter exceeds 32 bytes. Not reachable in practice:
/// `u32::MAX` yields `"A:4294967295"`, which is 12 bytes.
pub fn draw_status<D>(display: &mut D, counter_a: u32, counter_b: u32) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    let style_text = MonoTextStyle::new(&FONT_9X15, Rgb565::WHITE);

    let mut buf_a: heapless::String<32> = heapless::String::new();
    write!(buf_a, "A:{}", counter_a).unwrap();
    let _ = Text::with_alignment(
        &buf_a,
        Point::new(32, 80),
        style_text,
        Alignment::Center
    )
    .draw(display);

    let mut buf_b: heapless::String<32> = heapless::String::new();
    write!(buf_b, "B:{}", counter_b).unwrap();
    let _ = Text::with_alignment(
        &buf_b,
        Point::new(96, 80),
        style_text,
        Alignment::Center
    )
    .draw(display);

    Ok(())
}

/// Repaints the whole static backdrop: black background, red frame, "RUSTEESH"
/// title and the vertical separator between the two counter columns.
///
/// An expensive operation (the entire screen goes over SPI): keep it for
/// start-up, not for every counter update.
pub fn refresh_background(display: &mut impl DrawTarget<Color = Rgb565>) {
    let _ = display.clear(Rgb565::BLACK);

    let style_rect = PrimitiveStyleBuilder::new()
    .stroke_color(Rgb565::RED)
    .stroke_width(2)
    .build();

    let _ = Rectangle::new(Point::new(1, 1), Size::new(126, 158))
        .into_styled(style_rect)
        .draw(display);

    let style_text = MonoTextStyle::new(&FONT_9X15, Rgb565::WHITE);
    let _ = Text::with_alignment(
        "RUSTEESH",
        Point::new(64, 15), // Position at X = center of screen, Y = top offset (e.g., Y = 10)
        style_text,
        Alignment::Center,
    )
    .draw(display);

    let _ = Line::new(Point::new(64, 25), Point::new(64, 145))
        .into_styled(style_rect)
        .draw(display);
}

/// Stub display: draws nothing, only counts the pixels it receives. Lets the
/// rendering be checked without hardware, on a PC as well as on the board.
///
/// The pixel count is a coarse but sufficient measure: it establishes that a
/// function drew *something*, and allows two renders to be compared against each
/// other (a six-digit counter must produce more pixels than a one-digit one). It
/// obviously checks neither positions nor colours.
///
/// The `bounding_box` mirrors the real screen dimensions (128×160) so that the
/// clipping performed by `embedded-graphics` is identical.
pub struct FakeDrawTarget {
    pub nb: u32,
}

impl FakeDrawTarget {
    pub fn new() -> Self {
        Self { nb: 0 }
    }
}

impl Default for FakeDrawTarget {
    fn default() -> Self {
        Self::new()
    }
}

impl Dimensions for FakeDrawTarget {
    fn bounding_box(&self) -> Rectangle {
        Rectangle::new(Point::zero(), Size::new(128, 160))
    }
}

impl DrawTarget for FakeDrawTarget {
    type Color = Rgb565;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for _ in pixels {
            self.nb += 1;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics_simulator::SimulatorDisplay;

    #[test]
    fn draw_status_produces_pixels() {
        let mut d = FakeDrawTarget::new();
        draw_status(&mut d, 42, 24).unwrap();
        assert!(d.nb > 0);
    }

    #[test]
    fn wider_counters_draw_more_pixels() {
        let mut small = FakeDrawTarget::new();
        draw_status(&mut small, 1, 1).unwrap();

        let mut big = FakeDrawTarget::new();
        draw_status(&mut big, 100_000, 100_000).unwrap();

        assert!(big.nb > small.nb, "{} should exceed {}", big.nb, small.nb);
    }

    #[test]
    fn counters_never_overflow_the_buffer() {
        // `heapless::String<32>`: the `write!` panics on overflow.
        // u32::MAX gives "A:4294967295", 12 bytes — plenty of headroom.
        let mut d = FakeDrawTarget::new();
        draw_status(&mut d, u32::MAX, u32::MAX).unwrap();
    }

    #[test]
    fn refresh_background_clears_and_draws() {
        let mut d = FakeDrawTarget::new();
        refresh_background(&mut d);
        assert!(d.nb > 0);
    }

    #[test]
    fn draw_status_does_not_panic() {
        let mut display: SimulatorDisplay<Rgb565> = SimulatorDisplay::new(Size::new(160, 128));
        assert!(draw_status(&mut display, 42, 24).is_ok());
    }

    #[test]
    fn background_is_black_when_draw() {
        let mut display: SimulatorDisplay<Rgb565> = SimulatorDisplay::new(Size::new(160, 128));
        draw_status(&mut display, 42, 24).unwrap();

        // a pixel need must be not changed by draw_status
        let pixel = display.get_pixel(Point::new(5, 5));
        assert_eq!(pixel, Rgb565::BLACK);
    }
}
