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
//! | SPI2 SCK    | 23 | shared clock |
//! | SPI2 MOSI   | 22 | shared, display + radio |
//! | SPI2 MISO   | 21 | radio only, the ST7735s is write-only |
//! | Radio NSS   | 4  | chip select, active low |
//! | Radio RST   | 6  | reset, idles high |
//! | Radio BUSY  | 18 | input, driven by the SX1262 |
//! | Radio DIO1  | 7  | input, IRQ line |
//! | Radio RXEN  | 5  | RF switch, high = receive |
//!
//! The display and the radio **share SPI2**. They are arbitrated by a mutex and
//! told apart by their chip select; each gets its own clock frequency through
//! `SpiDeviceWithConfig`.
//!
//! The GPIO numbers cannot be constants: `peripherals.GPIO3` is a *field*,
//! consumed by move, not an index. They stay hard-coded inside [`Board::init`]
//! — which is precisely why they are isolated here.

use core::cell::RefCell;

use embassy_embedded_hal::shared_bus::blocking::spi::SpiDeviceWithConfig;
use embassy_sync::blocking_mutex::{raw::CriticalSectionRawMutex, Mutex};
use lora_phy::{iv::GenericSx126xInterfaceVariant, sx126x, sx126x::{Sx126x, Sx1262, TcxoCtrlVoltage}};
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

/// Display clock. The ST7735s is write-only and tolerates a fast bus.
const SPI_FREQ_DISPLAY_MHZ: u32 = 26;
/// Radio clock. The SX1262 datasheet caps SPI at 16 MHz; stay well under it.
const SPI_FREQ_RADIO_MHZ: u32 = 4;
const SPI_BUF_LEN: usize = 512;

/// SPI2, shared by the display and the radio.
///
/// Both peripherals sit on SCK/MOSI (GPIO23/22) and only differ by their chip
/// select, so the bus is behind a mutex and each user gets its own
/// [`SpiDeviceWithConfig`] — which re-applies its own clock before every
/// transaction. A plain `SpiDevice` would force both to share one frequency.
static SPI_BUS: StaticCell<SharedBus> = StaticCell::new();

type SharedBus = Mutex<CriticalSectionRawMutex, RefCell<Spi<'static, Blocking>>>;
type SharedSpi = SpiDeviceWithConfig<'static, CriticalSectionRawMutex, Spi<'static, Blocking>, Output<'static>>;

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
    SpiInterface<'static, SharedSpi, Output<'static>>,
    ST7735s,
    Output<'static>,
>;

/// The radio, wired but **not yet woken up**.
///
/// `LoRa::new` is `async` (it resets the chip and waits on BUSY), so it cannot
/// run in [`Board::init`]. The board therefore hands out the assembled
/// `Sx126x` and the caller finishes the job from a task:
///
/// ```ignore
/// let radio = LoRa::new(board.radio, false, embassy_time::Delay).await.unwrap();
/// ```
pub type Radio = Sx126x<
    AsyncSpiDevice<SharedSpi>,
    GenericSx126xInterfaceVariant<Output<'static>, Input<'static>>,
    Sx1262,
>;

// --- Blocking → async SPI shim ----------------------------------------------

/// Presents a blocking `SpiDevice` as an async one.
///
/// `lora-phy` demands [`embedded_hal_async::spi::SpiDevice`], but the bus is
/// shared with `mipidsi`, which is blocking-only — so the bus itself has to be
/// blocking, and this adapter bridges the gap by completing every transfer
/// synchronously.
///
/// The cost is real but small: the executor is stalled for the duration of a
/// transfer. SX1262 exchanges are a handful of bytes (a 255-byte FIFO read is
/// the worst case), which at 4 MHz is well under a millisecond. The long waits
/// — BUSY and DIO1 — stay genuinely async, since they go through `Wait` on the
/// GPIOs rather than through SPI.
pub struct AsyncSpiDevice<T>(pub T);

impl<T: embedded_hal::spi::ErrorType> embedded_hal::spi::ErrorType for AsyncSpiDevice<T> {
    type Error = T::Error;
}

impl<T: embedded_hal::spi::SpiDevice<u8>> embedded_hal_async::spi::SpiDevice<u8> for AsyncSpiDevice<T> {
    async fn transaction(
        &mut self,
        operations: &mut [embedded_hal::spi::Operation<'_, u8>],
    ) -> Result<(), Self::Error> {
        embedded_hal::spi::SpiDevice::transaction(&mut self.0, operations)
    }
}

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
    /// SX1262 radio, sharing SPI2 with the display. Still asleep — see
    /// [`Radio`] for how to bring it up.
    pub radio: Radio,
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

        // --- SPI2, shared by the display and the radio ---
        //
        // MISO is only needed by the radio, but it belongs to the bus, so it is
        // configured once here. The frequency set below is irrelevant: each
        // `SpiDeviceWithConfig` overrides it before every transaction.
        let spi = Spi::new(
            peripherals.SPI2,
            SpiConfig::default()
                .with_frequency(Rate::from_mhz(SPI_FREQ_DISPLAY_MHZ))
                .with_mode(SpiMode::_0),
        )
        .unwrap()
        .with_sck(peripherals.GPIO23)
        .with_mosi(peripherals.GPIO22)
        .with_miso(peripherals.GPIO21);

        let spi_bus = SPI_BUS.init(Mutex::new(RefCell::new(spi)));

        // --- ST7735s display, CS on GPIO1 ---
        let dc = Output::new(peripherals.GPIO8, Level::Low, OutputConfig::default());
        let rst = Output::new(peripherals.GPIO14, Level::High, OutputConfig::default());
        let cs = Output::new(peripherals.GPIO1, Level::High, OutputConfig::default());

        let display_device = SpiDeviceWithConfig::new(
            spi_bus,
            cs,
            SpiConfig::default()
                .with_frequency(Rate::from_mhz(SPI_FREQ_DISPLAY_MHZ))
                .with_mode(SpiMode::_0),
        );

        // The buffer has to be 'static: it is referenced by the `Display` we
        // return, so it cannot live on the stack.
        static BUF: StaticCell<[u8; SPI_BUF_LEN]> = StaticCell::new();
        let buffer = BUF.init([0; SPI_BUF_LEN]);

        let di = SpiInterface::new(display_device, dc, buffer);

        let display = Builder::new(ST7735s, di)
            .display_size(SCREEN_W, SCREEN_H)
            .reset_pin(rst)
            .orientation(Orientation::default())
            .init(&mut delay)
            .unwrap();

        // --- SX1262 radio, CS on GPIO4 ---
        //
        // RESET idles high: the chip is only held in reset during the reset
        // pulse, which `lora-phy` drives itself from `InterfaceVariant::reset`.
        // BUSY and DIO1 are actively driven by the radio, so no pull is needed.
        let nss = Output::new(peripherals.GPIO4, Level::High, OutputConfig::default());
        let reset = Output::new(peripherals.GPIO6, Level::High, OutputConfig::default());
        let busy = Input::new(peripherals.GPIO18, InputConfig::default());
        let dio1 = Input::new(peripherals.GPIO7, InputConfig::default());
        // RF switch, RX side only (`SX126X_RXEN` in the MeshCore config). The TX
        // side is driven by the radio itself through DIO2, so lora-phy is given
        // `None` for it. Without this pin the antenna is never routed to the
        // receiver and the radio listens to nothing.
        let rxen = Output::new(peripherals.GPIO5, Level::Low, OutputConfig::default());

        let radio_device = AsyncSpiDevice(SpiDeviceWithConfig::new(
            spi_bus,
            nss,
            SpiConfig::default()
                .with_frequency(Rate::from_mhz(SPI_FREQ_RADIO_MHZ))
                .with_mode(SpiMode::_0),
        ));

        let sx126x_config = sx126x::Config {
            chip: Sx1262,
            // 1.8 V, as documented for the `firebeetle_c6` variant in the
            // README. An under-driven TCXO drifts, and at BW 62.5 kHz a few kHz
            // of offset costs real sensitivity.
            tcxo_ctrl: Some(TcxoCtrlVoltage::Ctrl1V8),
            use_dcdc: false,
            rx_boost: true,
        };

        let iv = GenericSx126xInterfaceVariant::new(reset, dio1, busy, Some(rxen), None).unwrap();
        let radio = Sx126x::new(radio_device, iv, sx126x_config);

        Self { led, btn_a, btn_b, display, radio, timer: timg0.timer0, sw_int0: sw.software_interrupt0 }
    }
}
