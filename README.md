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
