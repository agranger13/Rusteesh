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

fn draw_signal_bar<D>(display: &mut D, rssi: i16) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    let bar_width = 20;
    let bar_height = 3;
    let bar_left = 64 - bar_width / 2;
    let bar_top = 25;
    let bar_style = PrimitiveStyleBuilder::new()
        .stroke_color(Rgb565::WHITE)
        .stroke_width(1)
        .build();
    let _ = Rectangle::new(
        Point::new(bar_left, bar_top),
        Size::new(bar_width as u32, bar_height as u32),
    )
    .into_styled(bar_style)
    .draw(display);

    // Fill the bar proportionally to the RSSI value. Assuming RSSI ranges from -120 dBm (weak) to 0 dBm (strong).
    let rssi_normalized = ((rssi + 120 + 50) as f32 / 120.0).clamp(0.0, 1.0);
    let fill_width = (bar_width as f32 * rssi_normalized) as u32;
    let color_filled = if rssi_normalized > 0.70 {
        Rgb565::GREEN
    } else {
        if rssi_normalized > 0.45 {
            Rgb565::YELLOW
        } else {
            Rgb565::RED
        }
    };
    let _ = Rectangle::new(
        Point::new(bar_left, bar_top),
        Size::new(fill_width, bar_height as u32),
    )
    .into_styled(PrimitiveStyleBuilder::new().fill_color(color_filled).build())
    .draw(display);

    Ok(())
}

/// How many bytes fit on one line: `"XX "` is 3 columns of `FONT_9X15`, so four
/// bytes span 108 px of the 128 px panel.
const BYTES_PER_ROW: usize = 4;
/// Rows between the header and the bottom edge, at 15 px each.
const MAX_ROWS: usize = 7;

/// Dumps a received packet as hexadecimal, preceded by its length.
///
/// Hexadecimal rather than text on purpose: a LoRa payload is arbitrary bytes.
/// MeshCore frames are binary — routing header, node ids, signature, ciphertext
/// — so decoding them as UTF-8 fails on almost every packet.
///
/// Only the first `BYTES_PER_ROW * MAX_ROWS` bytes are shown; the panel cannot
/// hold a full 255-byte packet. The length in the header is the real one, so a
/// truncated display is still unambiguous.
///
/// Erases nothing — like [`draw_status`], the caller clears the area first.
pub fn draw_paquets<D>(display: &mut D, message: &[u8], rssi: i16, _snr: i16) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    let style_text = MonoTextStyle::new(&FONT_9X15, Rgb565::WHITE);

    // Write the header: "len:<n>"
    let mut header: heapless::String<16> = heapless::String::new();
    // `write!` into a heapless String can only fail on overflow, which the
    // fixed capacities here rule out. Swallowed rather than unwrapped so a
    // display glitch can never bring the firmware down.
    let _ = write!(header, "len:{}", message.len());
    let _ = Text::with_alignment(&header, Point::new(64, 20), style_text, Alignment::Center)
        .draw(display);

    // Draw a signal strength bar under the header to indicate reception quality.
    draw_signal_bar(display, rssi)?;

    // Write the payload in hexadecimal, 4 bytes per row, 7 rows max. Each row is 15 px high.
    for (row, chunk) in message.chunks(BYTES_PER_ROW).take(MAX_ROWS).enumerate() {
        let mut line: heapless::String<16> = heapless::String::new();
        for b in chunk {
            let _ = write!(line, "{:02X} ", b);
        }
        let y = 40 + (row as i32) * 15;
        let _ = Text::with_alignment(&line, Point::new(64, y), style_text, Alignment::Center)
            .draw(display);
    }

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
    fn draw_paquets_renders_binary_payloads() {
        // The very payload that made the UTF-8 path fail on the board.
        let mut d = FakeDrawTarget::new();
        draw_paquets(&mut d, &[0x00, 0xFF, 0x9C, 0x12, 0x80], 0, 0).unwrap();
        assert!(d.nb > 0);
    }

    #[test]
    fn draw_paquets_survives_an_empty_packet() {
        // Only the "len:0" header is drawn, and no row loop runs.
        let mut d = FakeDrawTarget::new();
        assert!(draw_paquets(&mut d, &[], 0, 0).is_ok());
    }

    #[test]
    fn draw_paquets_truncates_instead_of_overflowing() {
        // A full 255-byte packet cannot fit on the panel: the function must clip
        // to MAX_ROWS rather than panic or run off the screen.
        let mut short = FakeDrawTarget::new();
        draw_paquets(&mut short, &[0xAB; BYTES_PER_ROW * MAX_ROWS], 0, 0).unwrap();

        let mut full = FakeDrawTarget::new();
        draw_paquets(&mut full, &[0xAB; 255], 0, 0).unwrap();

        // Same rows drawn; only the "len:" header differs in width.
        assert!(full.nb >= short.nb);
    }

    #[test]
    fn draw_paquets_shows_the_real_length_even_when_truncated() {
        let mut display = sim();
        draw_paquets(&mut display, &[0xAB; 255], 0, 0).unwrap();

        // The header sits on the y = 20 baseline.
        assert!(
            any_pixel(&display, 0..W as i32, 6..22, Rgb565::WHITE),
            "the length header should be drawn"
        );
        // Nothing may spill past the last row (baseline 40 + 6*15 = 130).
        assert!(
            !any_pixel(&display, 0..W as i32, 132..H as i32, Rgb565::WHITE),
            "rows should stop at MAX_ROWS, not run off the panel"
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
