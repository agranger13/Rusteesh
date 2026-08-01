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
    gpio::{Input, Output}, time::{Instant},
};

use rusteesh::board::{Board, Display};
use rusteesh::input::{Debouncer, PressEvent};
use rusteesh::ui::{draw_status, refresh_background};

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};

/// Monotonic clock in milliseconds, injected into `Debouncer::update`.
/// The only place that reads hardware time, so `input::Debouncer` stays pure.
fn now_ms(boot: Instant) -> u64 {
    boot.elapsed().as_millis()
}

enum Event {
    BoutonA,
    BoutonB,
}

static EVENTS: Channel<CriticalSectionRawMutex, Event, 8> = Channel::new();


#[esp_rtos::main]
async fn main(spawner: Spawner) {
    let board = Board::init();
    esp_rtos::start(board.timer, board.sw_int0);
    spawner.spawn(button_task(board.btn_a, board.btn_b, board.led).unwrap());
    spawner.spawn(display_task(board.display).unwrap());
}


#[embassy_executor::task]
async fn button_task(a: Input<'static>, b: Input<'static>, mut led: Output<'static>) {
    let mut button_a: Debouncer = Debouncer::new();
    let mut button_b: Debouncer = Debouncer::new();

    let boot = Instant::now();
    loop {
        let t = now_ms(boot);
        match button_a.update(a.is_low(), t) {
            PressEvent::Short => {
                led.set_high();
                Timer::after(Duration::from_millis(200)).await;
                led.set_low();
                EVENTS.send(Event::BoutonA).await;
                println!(">>> short A -> LED ON");
            }
            PressEvent::Long => {
                led.set_high();
                println!(">>> long A -> LED ON");
            }
            PressEvent::None => (),
        }

        match button_b.update(b.is_low(), t) {
            PressEvent::Short => {
                led.set_low();
                Timer::after(Duration::from_millis(200)).await;
                led.set_high();
                EVENTS.send(Event::BoutonB).await;
                println!(">>> short B -> LED OFF");
            }
            PressEvent::Long => {
                led.set_low();
                println!(">>> long B -> LED OFF");
            }
            PressEvent::None => (),
        }

        Timer::after(Duration::from_millis(2)).await;
    }
}


#[embassy_executor::task]
async fn display_task(mut display: Display) {
    let mut counter_a: u32 = 0;
    let mut counter_b: u32 = 0;

    refresh_background(&mut display);
    let _ = draw_status(&mut display, counter_a, counter_b);

    loop {
        let event = EVENTS.receive().await;
        match event {
            Event::BoutonA => {
                counter_a += 1;
                let _ = Rectangle::new(Point::new(10, 60), Size::new(40, 50))
                    .into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK))
                    .draw(&mut display);
                let _ = draw_status(&mut display, counter_a, counter_b);
            }
            Event::BoutonB => {
                counter_b += 1;
                let _ = Rectangle::new(Point::new(70, 60), Size::new(40, 50))
                    .into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK))
                    .draw(&mut display);
                let _ = draw_status(&mut display, counter_a, counter_b);
            }
        }
    }
}