//! Smoke check run at boot, on the board.
//!
//! The detailed unit tests live in each module's `#[cfg(test)] mod tests` and
//! run on a PC (`cargo test-host`). This module only keeps a smoke check:
//! confirming on real hardware that the embedded logic behaves as it does on the
//! host.

use esp_println::println;

use crate::input::{read_low_active, Debouncer, FakeDebouncer, PressEvent};
use crate::ui::{draw_status, FakeDrawTarget};

pub fn run() {
    let mut fake_btn = FakeDebouncer { pushed: true };
    assert!(read_low_active(&mut fake_btn));

    let mut button = Debouncer::new();
    for _ in 0..4 {
        assert!(matches!(button.update(true, 0), PressEvent::None));
    }
    assert!(matches!(button.update(true, 0), PressEvent::Short));
    assert!(matches!(button.update(true, 800), PressEvent::Long));

    let mut fake_disp = FakeDrawTarget::new();
    draw_status(&mut fake_disp, 42, 24).unwrap();
    assert!(fake_disp.nb > 0);

    println!("selftest ok ({} pixels drawn)", fake_disp.nb);
}
