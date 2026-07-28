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

struct Debouncer {
    etat: bool,
    compteur: u8,
}

impl Debouncer {
    const SEUIL: u8 = 5;

    fn new() -> Self {
        Self { etat: false, compteur: 0 }
    }

    /// Appelée toutes les ~2 ms. Retourne true au front descendant.
    fn update(&mut self, appuye: bool) -> bool {
        if appuye == self.etat {
            self.compteur = 0;
            return false;
        }
        self.compteur += 1;
        if self.compteur >= Self::SEUIL {
            self.compteur = 0;
            self.etat = appuye;
            return appuye; // front d'appui uniquement
        }
        false
    }
}

fn attendre_ms(ms: u64) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(ms) {}
}

#[main]
fn main() -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    let mut led = Output::new(peripherals.GPIO15, Level::Low, OutputConfig::default());
    let bouton_a = Input::new(peripherals.GPIO3, InputConfig::default().with_pull(Pull::Up));
    let bouton_b = Input::new(peripherals.GPIO17, InputConfig::default().with_pull(Pull::Up));
    let mut debouncer_a = Debouncer::new();
    let mut debouncer_b = Debouncer::new();

    loop {
        if debouncer_a.update(bouton_a.is_low()) {
            println!(">>> front A -> LED ON");
            led.set_high();
        }
        if debouncer_b.update(bouton_b.is_low()) {
            println!(">>> front B -> LED ON");
            led.set_low();
        }
        attendre_ms(2);
    }
}