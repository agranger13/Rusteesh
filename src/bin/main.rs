#![no_std]
#![no_main]

esp_bootloader_esp_idf::esp_app_desc!();

use esp_backtrace as _;
use esp_println::println;
use esp_hal::{
    clock::CpuClock,
    gpio::{Input, InputConfig, Pull, Output, OutputConfig, Level},
    main,
    time::{Duration, Instant},
};

enum PressEvent {Short, Long, None}

struct Button {
    state: bool,
    counter: u8,
    start: Option<Instant>,
    long_signaled: bool,
}

impl Button {
    const DEBOUNCE_THRESHOLD: u8 = 5;
    const LONG_PRESS_MS: u64 = 800;

    fn new() -> Self {
        Self {
            state: false,
            counter: 0,
            start: None,
            long_signaled: false,
        }
    }

    fn update(&mut self, pressed: bool) -> PressEvent {
        if pressed == self.state {
            self.counter = 0;

            if self.state {
                if let Some(t0) = self.start {
                    if !self.long_signaled && t0.elapsed() >= Duration::from_millis(Self::LONG_PRESS_MS) {
                        self.long_signaled = true;
                        return PressEvent::Long;
                    }
                }
            } else {
                self.start = None;
                self.long_signaled = false;
            }

            return PressEvent::None;
        }

        self.counter += 1;
        if self.counter >= Self::DEBOUNCE_THRESHOLD {
            self.counter = 0;
            self.state = pressed;

            if pressed {
                self.start = Some(Instant::now());
                self.long_signaled = false;
                return PressEvent::Short;
            }

            self.start = None;
            self.long_signaled = false;
            return PressEvent::None;
        }

        PressEvent::None
    }
}

fn wait_ms(ms: u64) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(ms) {}
}

#[main]
fn main() -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    let mut led = Output::new(peripherals.GPIO15, Level::Low, OutputConfig::default());
    let a = Input::new(peripherals.GPIO3, InputConfig::default().with_pull(Pull::Up));
    let b = Input::new(peripherals.GPIO17, InputConfig::default().with_pull(Pull::Up));
    let mut button_a = Button::new();
    let mut button_b = Button::new();

    loop {
        match button_a.update(a.is_low()) {
            PressEvent::Short => {
                led.set_high();
                wait_ms(200);
                led.set_low();
                println!(">>> short A -> LED ON");
            }
            PressEvent::Long => {
                led.set_high();
                println!(">>> long A -> LED ON");
            }
            PressEvent::None => (),
        }
        match button_b.update(b.is_low()) {
            PressEvent::Short => {
                led.set_low();
                wait_ms(200);
                led.set_high();
                println!(">>> short B -> LED OFF");
            }
            PressEvent::Long => {
                led.set_low();
                println!(">>> long B -> LED OFF");
            }
            PressEvent::None => (),
        }
        wait_ms(2);
    }
}