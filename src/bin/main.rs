//! Firmware entry point.
//!
//! Holds nothing but the event loop: bring-up is delegated to
//! [`rusteesh::board`], button logic to [`rusteesh::input`], rendering to
//! [`rusteesh::ui`]. No GPIO number appears here.
//!
//! # Loop
//!
//! Polls roughly every 2 ms. On each pass the raw state of each button and the
//! current clock are handed to `Debouncer::update`, which returns the event to
//! handle. This is the only place that reads hardware time — see [`now_ms`].
//!
//! # Known limitation
//!
//! The `wait_ms(200)` LED blinks are blocking: for those 200 ms the other button
//! is not polled and its presses are lost. Acceptable for now; worth revisiting
//! if responsiveness becomes a problem.

#![no_std]
#![no_main]

esp_bootloader_esp_idf::esp_app_desc!();

use esp_backtrace as _;
use esp_println::println;

use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
};
use esp_hal::{
    main,
    time::{Duration, Instant},
};

use rusteesh::board::Board;
use rusteesh::input::{Debouncer, PressEvent};
use rusteesh::selftest;
use rusteesh::ui::{draw_status, refresh_background};

/// Busy wait. Blocks the core for the whole duration: nothing else is polled
/// in the meantime.
fn wait_ms(ms: u64) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(ms) {}
}

/// Monotonic clock in milliseconds, injected into `Debouncer::update`.
/// The only place that reads hardware time, so `input::Debouncer` stays pure.
fn now_ms(boot: Instant) -> u64 {
    boot.elapsed().as_millis()
}

#[main]
fn main() -> ! {
    let mut board = Board::init();

    selftest::run();

    let mut button_a = Debouncer::new();
    let mut button_b = Debouncer::new();

    refresh_background(&mut board.display);

    let mut counter_a: u32 = 0;
    let mut counter_b: u32 = 0;
    let _ = draw_status(&mut board.display, counter_a, counter_b);

    let boot = Instant::now();

    loop {
        let t = now_ms(boot);

        match button_a.update(board.btn_a.is_low(), t) {
            PressEvent::Short => {
                board.led.set_high();
                wait_ms(200);
                board.led.set_low();
                println!(">>> short A -> LED ON");
                counter_a += 1;
                let _ = Rectangle::new(Point::new(10, 60), Size::new(40, 50))
                    .into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK))
                    .draw(&mut board.display);
                let _ = draw_status(&mut board.display, counter_a, counter_b);
            }
            PressEvent::Long => {
                board.led.set_high();
                println!(">>> long A -> LED ON");
            }
            PressEvent::None => (),
        }

        match button_b.update(board.btn_b.is_low(), t) {
            PressEvent::Short => {
                board.led.set_low();
                wait_ms(200);
                board.led.set_high();
                println!(">>> short B -> LED OFF");
                counter_b += 1;
                let _ = Rectangle::new(Point::new(70, 60), Size::new(40, 50))
                    .into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK))
                    .draw(&mut board.display);
                let _ = draw_status(&mut board.display, counter_a, counter_b);
            }
            PressEvent::Long => {
                board.led.set_low();
                println!(">>> long B -> LED OFF");
            }
            PressEvent::None => (),
        }

        wait_ms(2);
    }
}
