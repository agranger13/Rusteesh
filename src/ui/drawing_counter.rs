//! Rendering of the button counters.
//!
//! Nothing here knows about the ST7735s: everything goes through `DrawTarget`,
//! which is what lets the tests below run these very functions against a
//! `SimulatorDisplay` on a PC.

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

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::primitives::PrimitiveStyle;
    use embedded_graphics_simulator::SimulatorDisplay;

    /// Stub display: draws nothing, only counts the pixels it receives.
    ///
    /// The pixel count is a coarse measure: it establishes that a function drew
    /// *something*, and allows two renders to be compared against each other (a
    /// six-digit counter must produce more pixels than a one-digit one). It
    /// checks neither positions nor colours — that is what `SimulatorDisplay`
    /// is used for below.
    ///
    /// The `bounding_box` mirrors the real panel (128×160) so that the clipping
    /// performed by `embedded-graphics` is identical.
    struct FakeDrawTarget {
        nb: u32,
    }

    impl FakeDrawTarget {
        fn new() -> Self {
            Self { nb: 0 }
        }
    }

    impl Dimensions for FakeDrawTarget {
        fn bounding_box(&self) -> Rectangle {
            Rectangle::new(Point::zero(), Size::new(W, H))
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

    // Same dimensions as the real panel (see `board::SCREEN_W` / `SCREEN_H`),
    // otherwise the position assertions below would be meaningless.
    const W: u32 = 128;
    const H: u32 = 160;

    fn sim() -> SimulatorDisplay<Rgb565> {
        SimulatorDisplay::new(Size::new(W, H))
    }

    /// Is any pixel of `color` present in the `x` / `y` ranges (end-exclusive)?
    fn any_pixel(
        d: &SimulatorDisplay<Rgb565>,
        xs: core::ops::Range<i32>,
        ys: core::ops::Range<i32>,
        color: Rgb565,
    ) -> bool {
        ys.flat_map(|y| xs.clone().map(move |x| Point::new(x, y)))
            .any(|p| d.get_pixel(p) == color)
    }

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
        let mut display = sim();
        assert!(draw_status(&mut display, 42, 24).is_ok());
    }

    #[test]
    fn draw_status_leaves_the_rest_of_the_screen_alone() {
        let mut display = sim();
        draw_status(&mut display, 42, 24).unwrap();

        // Documented contract: draw_status erases nothing, so a pixel far from
        // the counters must stay untouched.
        assert_eq!(display.get_pixel(Point::new(5, 5)), Rgb565::BLACK);
    }

    // --- Below: what FakeDrawTarget cannot check — colours and positions ---

    #[test]
    fn counter_a_lands_left_and_b_lands_right() {
        let mut display = sim();
        draw_status(&mut display, 1, 2).unwrap();

        // The text sits on the y = 80 baseline; look a little above it.
        let band = 66..82;
        let midline = (W / 2) as i32;

        assert!(
            any_pixel(&display, 0..midline, band.clone(), Rgb565::WHITE),
            "counter A should be drawn in the left half"
        );
        assert!(
            any_pixel(&display, midline..W as i32, band, Rgb565::WHITE),
            "counter B should be drawn in the right half"
        );
    }

    #[test]
    fn refresh_background_paints_a_red_frame() {
        let mut display = sim();
        refresh_background(&mut display);

        // Top and bottom edges of the frame.
        assert!(any_pixel(&display, 0..W as i32, 0..4, Rgb565::RED));
        assert!(any_pixel(&display, 0..W as i32, H as i32 - 4..H as i32, Rgb565::RED));
    }

    #[test]
    fn refresh_background_clears_to_black_first() {
        let mut display = sim();

        // Dirty a pixel inside the frame, away from anything drawn.
        Rectangle::new(Point::new(20, 120), Size::new(4, 4))
            .into_styled(PrimitiveStyle::with_fill(Rgb565::GREEN))
            .draw(&mut display)
            .unwrap();
        assert_eq!(display.get_pixel(Point::new(21, 121)), Rgb565::GREEN);

        refresh_background(&mut display);
        assert_eq!(display.get_pixel(Point::new(21, 121)), Rgb565::BLACK);
    }
}
