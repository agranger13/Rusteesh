//! Wiring and hardware bring-up — **the only module that knows about the
//! board**.
//!
//! The rest of the firmware works through traits (`DrawTarget`, `InputPin`) or
//! plain types. Changing the pinout, the SPI frequency or the display should
//! therefore only ever touch this file.
//!
//! # Pinout (ESP32-C6)
//!
//! | Function | GPIO | Notes |
//! |---|---|---|
//! | LED         | 15 | output, active high |
//! | Button A    | 3  | input, internal pull-up, active low |
//! | Button B    | 17 | input, internal pull-up, active low |
//! | Display DC  | 8  | data/command |
//! | Display RST | 14 | reset, idles high |
//! | Display CS  | 1  | chip select, active low |
//! | Display SCK | 23 | SPI2 clock |
//! | Display MOSI| 22 | SPI2 data |
//!
//! The GPIO numbers cannot be constants: `peripherals.GPIO3` is a *field*,
//! consumed by move, not an index. They stay hard-coded inside [`Board::init`]
//! — which is precisely why they are isolated here.

use embedded_hal_bus::spi::ExclusiveDevice;
use static_cell::StaticCell;
use esp_hal::{interrupt::software::{SoftwareInterrupt, SoftwareInterruptControl}, timer::timg::{Timer, TimerGroup}};
use mipidsi::{
    interface::SpiInterface,
    models::ST7735s,
    options::Orientation,
    Builder,
};
use esp_hal::{
    clock::CpuClock,
    delay::Delay,
    gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull},
    spi::{
        master::{Config as SpiConfig, Spi},
        Mode as SpiMode,
    },
    time::Rate,
    Blocking,
};

// --- Hardware configuration --------------------------------------------------

pub const SCREEN_W: u16 = 128;
pub const SCREEN_H: u16 = 160;

const SPI_FREQ_MHZ: u32 = 26;
const SPI_BUF_LEN: usize = 512;

/// The concrete display type, with the whole SPI stack resolved.
///
/// A struct field needs a known size, so it cannot hold
/// `DrawTarget<Color = Rgb565>`, which is a trait. This type *implements* that
/// trait, and the trait is what `ui::` manipulates through its generics.
///
/// # Why an alias
///
/// The full type stacks four layers of generics (`mipidsi` over `SpiInterface`
/// over `ExclusiveDevice` over `Spi`). Spelling it out once here avoids having
/// to repeat it in every signature.
///
/// Tip for rebuilding it after a change (different display, different bus):
/// write `let _: () = display;` and the compiler will print the exact expected
/// type in its error message.
///
/// # Alternatives ruled out
///
/// * `Box<dyn DrawTarget<…>>` — impossible: `draw_iter<I>` is generic, so the
///   trait is not *dyn-safe*. And there is no allocator under `no_std`.
/// * `impl DrawTarget<…>` as a field — rejected by the language, `impl Trait` is
///   only allowed in argument or return position.
/// * `Board<D: DrawTarget<…>>` — compiles, but `init()` can only ever build one
///   `D`, so it would cost a viral generic parameter for nothing.
pub type Display = mipidsi::Display<
    SpiInterface<
        'static,
        ExclusiveDevice<Spi<'static, Blocking>, Output<'static>, Delay>,
        Output<'static>,
    >,
    ST7735s,
    Output<'static>,
>;

// --- Board -------------------------------------------------------------------

/// Every peripheral on the board, already initialised.
///
/// The fields are public because `main` borrows them independently (`board.led`
/// and `board.display` at the same time, for instance), which the borrow
/// checker accepts without complaint since they are distinct fields.
pub struct Board {
    /// Base de temps de l'ordonnanceur, à passer à `esp_rtos::start`.
    pub timer: Timer<'static>,
    /// Interruption logicielle 0, utilisée pour les changements de contexte.
    pub sw_int0: SoftwareInterrupt<'static, 0>,
    /// On-board LED, GPIO15.
    pub led: Output<'static>,
    /// Button A, GPIO3. Active low: use `btn_a.is_low()`.
    pub btn_a: Input<'static>,
    /// Button B, GPIO17. Active low.
    pub btn_b: Input<'static>,
    /// 128×160 ST7735s display, ready for `embedded-graphics` primitives.
    pub display: Display,
}

impl Board {
    /// Brings up the SoC and every peripheral.
    ///
    /// Sets the CPU clock to its maximum, configures the GPIOs, sets up the SPI2
    /// bus and runs the ST7735s init sequence (hardware reset included).
    ///
    /// # Panics
    ///
    /// Must be called **exactly once**: `esp_hal::init` panics on a second call,
    /// since `Peripherals` is a singleton. That is also why this function takes
    /// no `Peripherals` argument — it is the single entry point to the hardware.
    ///
    /// Also panics if the SPI configuration is invalid or if display init fails:
    /// without a display, the firmware has nothing to do.
    pub fn init() -> Self {
        let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
        let peripherals = esp_hal::init(config);
        let timg0 = TimerGroup::new(peripherals.TIMG0);
        let sw = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);

        let mut delay = Delay::new();

        let led = Output::new(peripherals.GPIO15, Level::Low, OutputConfig::default());

        let btn_a = Input::new(peripherals.GPIO3, InputConfig::default().with_pull(Pull::Up));
        let btn_b = Input::new(peripherals.GPIO17, InputConfig::default().with_pull(Pull::Up));

        // --- ST7735s display on SPI2 ---
        let dc = Output::new(peripherals.GPIO8, Level::Low, OutputConfig::default());
        let rst = Output::new(peripherals.GPIO14, Level::High, OutputConfig::default());
        let cs = Output::new(peripherals.GPIO1, Level::High, OutputConfig::default());

        let spi = Spi::new(
            peripherals.SPI2,
            SpiConfig::default()
                .with_frequency(Rate::from_mhz(SPI_FREQ_MHZ))
                .with_mode(SpiMode::_0),
        )
        .unwrap()
        .with_sck(peripherals.GPIO23)
        .with_mosi(peripherals.GPIO22);

        let spi_device = ExclusiveDevice::new(spi, cs, Delay::new()).unwrap();

        // The buffer has to be 'static: it is referenced by the `Display` we
        // return, so it cannot live on the stack.
        static BUF: StaticCell<[u8; SPI_BUF_LEN]> = StaticCell::new();
        let buffer = BUF.init([0; SPI_BUF_LEN]);

        let di = SpiInterface::new(spi_device, dc, buffer);

        let display = Builder::new(ST7735s, di)
            .display_size(SCREEN_W, SCREEN_H)
            .reset_pin(rst)
            .orientation(Orientation::default())
            .init(&mut delay)
            .unwrap();

        Self { led, btn_a, btn_b, display, timer: timg0.timer0, sw_int0: sw.software_interrupt0 }
    }
}
