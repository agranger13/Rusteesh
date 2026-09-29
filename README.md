# Rusteesh

`no_std` Rust firmware for the ESP32-C6: two buttons, one LED, and a 128×160
ST7735s TFT display over SPI. Each press increments a counter shown on screen.

## Commands

| Command | Effect |
|---|---|
| `cargo build` | Builds the firmware for the board |
| `cargo run` | Builds, flashes and opens the serial monitor (`espflash`) |
| `cargo test-host` | Runs the unit tests **on the PC** |
| `cargo sim` | Renders the UI in a desktop window (SDL2), no board needed |
| `cargo doc --open` | Generates and opens the documentation |

### Why `cargo test-host` and not `cargo test`

`.cargo/config.toml` pins `target = "riscv32imac-unknown-none-elf"` for every
cargo command, and that setting cannot be overridden per subcommand. A plain
`cargo test` would therefore produce a RISC-V binary that no runner can execute
on the PC.

The `test-host` alias is simply:

```
cargo test --target x86_64-pc-windows-msvc --no-default-features
```

`--no-default-features` turns off the `hw` feature, so no `esp-*` crate is
compiled — they only target RISC-V and would supply a `#[panic_handler]` that
conflicts with the one from `std`.

### The simulator and SDL2

`cargo sim` opens an actual window through `embedded-graphics-simulator`, which
links against SDL2 — a C library. Rather than installing it by hand, the `sdl2`
dev-dependency carries the `bundled` and `static-link` features: SDL2 is built
from source (cmake required) and folded into the binary, so there is no
`SDL2.lib` to place and no `SDL2.dll` to ship.

CMake 4 removed support for projects declaring `cmake_minimum_required` below
3.5, which SDL2 still does. `.cargo/config.toml` sets
`CMAKE_POLICY_VERSION_MINIMUM = "3.5"` to configure it anyway. Without it the
build fails with *"Compatibility with CMake < 3.5 has been removed"*.

The simulator deliberately lives in `examples/`, not in a `#[test]`: it blocks
until the window is closed, so it would hang the test suite and fail on a
headless machine.

### Phantom errors in the editor

If rust-analyzer reports `can't find crate for std`, `cannot find attribute
test` or `cannot find macro write` while `cargo build` succeeds, it is analysing
the crate with `--all-targets` (hence `cfg(test)`) against the RISC-V target — a
combination no real build ever uses. `.vscode/settings.json` disables that; see
the comments in it to switch rust-analyzer over to the tests instead.

## Architecture

One rule: **only one module knows about the hardware**.

```
src/
├── board.rs      ← THE hardware module: GPIO, SPI, display bring-up
├── input/        ← button logic (bool + u64, nothing else)
├── ui/           ← rendering, generic over DrawTarget
└── bin/main.rs   ← event loop
```

| Module | Depends on `esp-hal`? | Tested on a PC? |
|---|---|---|
| `board` | yes — that is its job | no |
| `input` | no | yes |
| `ui` | no | yes |

### The two dependency inversions

Two places where the hardware would naturally have leaked into the logic:

**Time.** `Debouncer` does not read the clock; the caller injects it:
`update(pressed, now_ms)`. A test can therefore simulate an 800 ms press
instantly instead of really waiting. Only `main` calls `Instant::now()`.

**The display.** `ui::` is generic over `DrawTarget<Color = Rgb565>`. The same
function drives the real screen on the board and a `SimulatorDisplay` under
`cargo test-host`, so the tests can inspect actual pixels — colours and
positions included.

Without those two, `cargo test-host` would be impossible: `input` and `ui` would
drag in a dependency on `esp-hal`, which does not build for x86.

## Pinout (ESP32-C6)

| Function | GPIO | Notes |
|---|---|---|
| LED | 15 | output, active high |
| Button A | 3 | internal pull-up, active low |
| Button B | 17 | internal pull-up, active low |
| Display DC | 8 | data/command |
| Display RST | 14 | reset |
| Display CS | 1 | chip select |
| Display SCK | 23 | SPI2, 26 MHz, mode 0 |
| Display MOSI | 22 | SPI2 |

Everything is concentrated in `Board::init()` — the only file to touch when
rewiring.

## Implementation notes worth knowing

**Debouncing.** A state change must be confirmed by five consecutive reads
(`Debouncer` in `src/input/debouncer.rs`). The
threshold is in *number of reads*, not milliseconds: with the 2 ms loop that
works out to about 10 ms of filtering.

**`Short` is emitted on press, not on release.** A long press therefore produces
`Short` followed by `Long` 800 ms later. To genuinely tell the two apart, move
the `Short` emission to the release edge, guarded by `!long_signaled`.

**The SPI buffer is a `StaticCell`.** `Board::init()` returns a `Display` that
references this buffer; a local array on the stack would be destroyed when the
function returns. `StaticCell` yields a `&'static mut` without `unsafe`.

**`build.rs` is gated on `target_os = "none"`.** It emits linker arguments
(`-Tlinkall.x`) that the MSVC linker does not understand and that caused
`LNK4044` warnings during host tests.

**The loop blocks for 200 ms** on LED blinks, during which the other button is
not polled.


# Info MeshCore
Pin used :
  -D ARDUINO_USB_CDC_ON_BOOT=1
  -D ARDUINO_USB_MODE=1
  -D P_LORA_TX_LED=15
  -D P_LORA_SCLK=23
  -D P_LORA_MISO=21
  -D P_LORA_MOSI=22
  -D P_LORA_NSS=4
  -D P_LORA_DIO_1=7
  -D P_LORA_BUSY=18
  -D P_LORA_RESET=6
  -D SX126X_RXEN=5

Effective radio parameters (EU/UK Narrow preset), traced back to the MeshCore
sources:

| Parameter | Value | Source |
|---|---|---|
| Frequency | 869.618 MHz | `platformio.ini:28` |
| Bandwidth (BW) | 62.5 kHz | `platformio.ini:29` |
| Spreading factor (SF) | 8 | `platformio.ini:30` |
| Coding rate (CR) | **8 → 4/8** (effective, as read in the app) | persisted prefs; compile-time default is 5 (`MyMesh.h:49`) |
| Sync word | 0x12 (private) — 0x1424 in the SX1262 register | `CustomSX1262.h:45` |
| Preamble length | 32 symbols | `RadioLibWrappers.h:47` |
| TX power | 14 dBm | `firebeetle_c6` variant |
| TCXO (DIO3) | 1.8 V | `firebeetle_c6` variant |
| Average Paquets Number | 2/min | At step 5.2, running during 35min, see 75 paquets |


Three traps when reimplementing this:

- **Compile-time values are not authoritative.** `MyMesh.cpp:877` seeds `_prefs`
  from the `LORA_*` defines once; after that the app pushes its own settings
  through `CMD_SET_RADIO_PARAMS` (`MyMesh.cpp:1385`), which calls `savePrefs()`
  then `radio_driver.setParams()`. Those prefs go to flash and are reloaded on
  every boot. **The app shows what is actually on the air** — hence CR=8 even
  though the firmware compiles with 5. Accepted ranges are `cr ∈ [5,8]`,
  `sf ∈ [5,12]`, `bw ∈ [7.8, 500]`.

- **The preamble is not the one passed to `begin()`.** `CustomSX1262.h:45`
  passes 16, but `RadioLibWrapper::begin()` overwrites it right away with
  `preambleLengthForSF(sf) = sf <= 8 ? 32 : 16` (`RadioLibWrappers.cpp:30`). With
  SF=8 the effective value is therefore **32**. It is reapplied after every
  transmission (`onSendFinished`).

- **Sync word 0x12 is the SX127x notation.** The SX1262 encodes it over 16 bits
  in its register, i.e. **0x1424**. That is the value to write when programming
  the register directly.