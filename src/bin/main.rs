#![no_std]
#![no_main]

esp_bootloader_esp_idf::esp_app_desc!();

use esp_backtrace as _;
use esp_println::println;
use embedded_hal::{digital::InputPin};
use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_graphics::{
    mono_font::{ascii::FONT_9X15, MonoTextStyle},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::Text,
};
use mipidsi::{
    interface::SpiInterface,
    models::ST7735s,
    options::{ColorOrder, Orientation},
    Builder,
};
use esp_hal::{
    clock::CpuClock,
    delay::Delay,
    gpio::{Input, InputConfig, Pull, Output, OutputConfig, Level},
    main,
    spi::{
        master::{Config as SpiConfig, Spi},
        Mode as SpiMode,
    },
    time::{Duration, Instant, Rate},
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

struct FakeButton {
    pushed: bool,
}

impl embedded_hal::digital::ErrorType for FakeButton {
    type Error = core::convert::Infallible;
}

impl InputPin for FakeButton {
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        Ok(!self.pushed)
    }
    fn is_low(&mut self) -> Result<bool, Self::Error> {
        Ok(self.pushed)
    }
}

fn read_low_active<P>(pin: &mut P) -> bool 
where
    P: InputPin,
{
    return pin.is_low().unwrap_or(false);
}

fn wait_ms(ms: u64) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(ms) {}
}

#[main]
fn main() -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let mut delay = Delay::new();

    let mut led = Output::new(peripherals.GPIO15, Level::Low, OutputConfig::default());

    let a = Input::new(peripherals.GPIO3, InputConfig::default().with_pull(Pull::Up));
    let b = Input::new(peripherals.GPIO17, InputConfig::default().with_pull(Pull::Up));
    let mut button_a = Button::new();
    let mut button_b = Button::new();

    let mut test_button = FakeButton { pushed: true };
    assert!(read_low_active(&mut test_button));

    // Define SPI configuration
    let dc = Output::new(peripherals.GPIO8, Level::Low, OutputConfig::default());
    let mut rst = Output::new(peripherals.GPIO14, Level::High, OutputConfig::default());
    let cs = Output::new(peripherals.GPIO1, Level::High, OutputConfig::default());

    let spi = Spi::new(
        peripherals.SPI2, 
        SpiConfig::default()
            .with_frequency(Rate::from_mhz(26))
            .with_mode(SpiMode::_0)
        )
        .unwrap()
        .with_sck(peripherals.GPIO23)
        .with_mosi(peripherals.GPIO22);

    let spi_device = ExclusiveDevice::new(spi, cs, delay).unwrap();

    let mut buffer = [0_u8; 512];
    let di = SpiInterface::new(spi_device, dc, &mut buffer);

    let mut display = Builder::new(ST7735s, di)
        .display_size(128, 160)
        .reset_pin(&mut rst)
        .orientation(Orientation::default())
        .init(&mut delay)
        .unwrap();

    display.clear(Rgb565::BLACK).unwrap();

    let style = MonoTextStyle::new(&FONT_9X15, Rgb565::WHITE);
    Text::new("Hello World!", Point::new(10, 30), style)
        .draw(&mut display)
        .unwrap();


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
