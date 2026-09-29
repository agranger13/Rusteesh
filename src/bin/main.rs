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

use heapless::Vec;
use lora_phy::{LoRa, RxMode, mod_params::{Bandwidth, CodingRate, SpreadingFactor}};
use rusteesh::board::{Board, Display};
use rusteesh::input::{Debouncer, PressEvent};
use rusteesh::ui::{draw_status, draw_paquets,refresh_background};
use rusteesh::radio::messages::{MessageQueue, Message};

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_executor::Spawner;
use embassy_time::{with_timeout, Duration, TimeoutError, Timer};

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
static PAQUETS: Channel<CriticalSectionRawMutex, Message, 8> = Channel::new();


#[esp_rtos::main]
async fn main(spawner: Spawner) {
    let board = Board::init();
    esp_rtos::start(board.timer, board.sw_int0);
    spawner.spawn(button_task(board.btn_a, board.btn_b).unwrap());
    // spawner.spawn(display_counter_task(board.display).unwrap());
    spawner.spawn(display_lora_task(board.display).unwrap());
    spawner.spawn(blink_task(board.led).unwrap());
    spawner.spawn(radio_task(board.radio).unwrap());
}


#[embassy_executor::task]
async fn button_task(a: Input<'static>, b: Input<'static>) {
    let mut button_a: Debouncer = Debouncer::new();
    let mut button_b: Debouncer = Debouncer::new();

    let boot = Instant::now();
    loop {
        let t = now_ms(boot);
        match button_a.update(a.is_low(), t) {
            PressEvent::Short => {
                EVENTS.send(Event::BoutonA).await;
                println!(">>> short A -> LED ON");
            }
            PressEvent::Long => {
                println!(">>> long A -> LED ON");
            }
            PressEvent::None => (),
        }

        match button_b.update(b.is_low(), t) {
            PressEvent::Short => {
                EVENTS.send(Event::BoutonB).await;
                println!(">>> short B -> LED OFF");
            }
            PressEvent::Long => {
                println!(">>> long B -> LED OFF");
            }
            PressEvent::None => (),
        }

        Timer::after(Duration::from_millis(2)).await;
    }
}


#[embassy_executor::task]
async fn display_counter_task(mut display: Display) {
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

#[embassy_executor::task]
async fn display_lora_task(mut display: Display) {
    let _ = display.clear(Rgb565::BLACK);
    loop {
        let event = PAQUETS.receive().await;
        match event {
            Message::Packet(packet, rssi, snr) => {
                let _ = display.clear(Rgb565::BLACK);
                let _ = draw_paquets(&mut display, &packet, rssi, snr);
            }

        }
    }
    
}

#[embassy_executor::task]
async fn blink_task (mut led: Output<'static>) {
    led.set_low();
    loop {
        led.toggle();
        Timer::after(Duration::from_millis(1000)).await;
    }
}

#[embassy_executor::task]
async fn radio_task (radio_kind: rusteesh::board::Radio) {
    // A mute SX1262 does not report an error: it holds BUSY high and
    // `LoRa::new` awaits it forever. The timeout turns that silent hang into a
    // message — the single most likely cause is a wrong `tcxo_ctrl`.
    println!("radio: waking up…");
    let mut lora = match with_timeout(
        Duration::from_secs(2),
        LoRa::new(radio_kind, false, embassy_time::Delay),
    )
    .await
    {
        Ok(Ok(l)) => { println!("radio: awake"); l }
        Ok(Err(e)) => { println!("radio: init failed {:?}", e); return; }
        Err(TimeoutError) => {
            println!("radio: BUSY stuck high — check tcxo_ctrl and the SPI wiring");
            return;
        }
    };

    let modulation_params = {
        match lora.create_modulation_params(
            SpreadingFactor::_8,
            Bandwidth::_62KHz,
            CodingRate::_4_8,
            869_618_000,
        ) {
            Ok(mp) => mp,
            Err(err) => {
                println!("radio modulcationinit failed: {:?}", err);
                return;
            }
        }
    };

    let rx_packet_params = {
        // 32 symbols, not the 16 MeshCore passes to `begin()`: RadioLibWrapper
        // overwrites it with `preambleLengthForSF(8) = 32`. See the README.
        match lora.create_rx_packet_params(32, false, 255, true, false, &modulation_params) {
            Ok(pp) => pp,
            Err(err) => {
                println!("radio rx init failed: {:?}", err);
                return;
            }
        }
    };

    // Receive loop
    let mut buffer = [0u8; 255];
    if let Err(err) = lora
        .prepare_for_rx(RxMode::Continuous, &modulation_params, &rx_packet_params)
        .await
    {
        println!("radio: prepare_for_rx failed: {:?}", err);
        return;
    }

    let mut deque = MessageQueue::new();

    println!("radio: listening on 869.618 MHz — silence from here on is normal");

    loop {
        match lora.rx(&rx_packet_params, &mut buffer).await {
            Ok((len, status)) => {
                // buffer[..len] contient le paquet
                // status.rssi, status.snr
                println!("Received packet: len={}, rssi={}, snr={}", len, status.rssi, status.snr);
                let payload = Vec::from_slice(&buffer[..len as usize]).unwrap();
                let message = Message::Packet(payload, status.rssi, status.snr);
                PAQUETS.send(message).await;

                deque.purger();
                deque.enregistrer();
                println!("Paquets per minute: {}/min", deque.par_minute());
            }
            Err(err) => {
                // journaliser, mais surtout ne pas paniquer
                println!("radio rx read failed: {:?}", err);
                Timer::after(Duration::from_millis(1000)).await;
            }
        }
    }
}