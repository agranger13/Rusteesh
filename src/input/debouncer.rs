use embedded_hal::digital::InputPin;

/// Event produced by [`Debouncer::update`] on each loop iteration.
///
/// # What a real press looks like
///
/// Note that `Short` is emitted **on press**, not on release. A long press
/// therefore produces both events:
///
/// ```text
/// press ─────────────────────── release
///   │                              │
///   ▼                              ▼
/// Short          Long (at +800 ms)  (nothing)
/// ```
///
/// A brief press only produces `Short`. To genuinely tell the two apart, move
/// the `Short` emission to the release edge, guarded by `!long_signaled`.
pub enum PressEvent {
    /// The button has just been pressed (once debouncing has confirmed it).
    Short,
    /// The button has been held for 800 ms. Emitted **once** per press.
    Long,
    /// Nothing to report on this iteration.
    None,
}

/// Button state machine: debouncing plus long-press detection.
///
/// Named after its main job, but it also owns the press/long-press timing —
/// it is the whole logical view of one button.
///
/// Depends on no peripheral: time is **injected** by the caller through
/// `now_ms`. That is what makes the logic testable on a PC, and what allows a
/// long press to be simulated without actually waiting 800 ms.
///
/// # Debouncing
///
/// A mechanical contact "bounces" for a few milliseconds on both make and
/// break, producing spurious transitions. The countermeasure here is a counter:
/// a state change must be **confirmed by 5 consecutive reads**
/// (`DEBOUNCE_THRESHOLD`) before it is accepted. Any read that contradicts the
/// trend resets the counter to zero.
///
/// The threshold is expressed in *number of reads*, not milliseconds, so its
/// real duration depends on the caller's loop period. At the 2 ms used by
/// `main`, it works out to roughly 10 ms of filtering.
///
/// # Usage
///
/// Call it on every loop iteration, even when nothing changes — long-press
/// detection relies on those repeated calls:
///
/// ```
/// use rusteesh::input::{Debouncer, PressEvent};
///
/// let mut button = Debouncer::new();
///
/// // Five consecutive reads are needed to confirm the press.
/// for _ in 0..4 {
///     assert!(matches!(button.update(true, 0), PressEvent::None));
/// }
/// assert!(matches!(button.update(true, 0), PressEvent::Short));
///
/// // Held for 800 ms: the long press fires, exactly once.
/// assert!(matches!(button.update(true, 800), PressEvent::Long));
/// assert!(matches!(button.update(true, 5_000), PressEvent::None));
/// ```
pub struct Debouncer {
    state: bool,
    counter: u8,
    start_ms: Option<u64>,
    long_signaled: bool,
}

impl Debouncer {
    const DEBOUNCE_THRESHOLD: u8 = 5;
    const LONG_PRESS_MS: u64 = 800;

    pub fn new() -> Self {
        Self {
            state: false,
            counter: 0,
            start_ms: None,
            long_signaled: false,
        }
    }

    /// Advances the state machine by one step. Call it on every loop iteration,
    /// including when the state has not changed.
    ///
    /// * `pressed` — raw input state, already normalised (`true` = pressed).
    ///   For an active-low button that is `pin.is_low()` — see
    ///   [`read_low_active`].
    /// * `now_ms` — monotonic clock in milliseconds. It must be non-decreasing
    ///   and consistent across calls; its origin is irrelevant, only the
    ///   differences matter.
    ///
    /// Returns at most one event per call: if a press is confirmed and the
    /// long-press deadline has already elapsed, `Short` comes out first and
    /// `Long` waits for the next iteration.
    pub fn update(&mut self, pressed: bool, now_ms: u64) -> PressEvent {
        if pressed == self.state {
            self.counter = 0;

            if self.state {
                if let Some(t0) = self.start_ms {
                    if !self.long_signaled && now_ms.saturating_sub(t0) >= Self::LONG_PRESS_MS {
                        self.long_signaled = true;
                        return PressEvent::Long;
                    }
                }
            } else {
                self.start_ms = None;
                self.long_signaled = false;
            }

            return PressEvent::None;
        }

        self.counter += 1;
        if self.counter >= Self::DEBOUNCE_THRESHOLD {
            self.counter = 0;
            self.state = pressed;

            if pressed {
                self.start_ms = Some(now_ms);
                self.long_signaled = false;
                return PressEvent::Short;
            }

            self.start_ms = None;
            self.long_signaled = false;
            return PressEvent::None;
        }

        PressEvent::None
    }
}

impl Default for Debouncer {
    fn default() -> Self {
        Self::new()
    }
}

/// Stub `InputPin`, used to exercise active-low reads without hardware.
///
/// Unrelated to [`Debouncer`]: it stands in for the *pin*, not for the
/// debouncing logic. Its only user is [`read_low_active`].
pub struct FakePin {
    pub pushed: bool,
}

impl embedded_hal::digital::ErrorType for FakePin {
    type Error = core::convert::Infallible;
}

impl InputPin for FakePin {
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        Ok(!self.pushed)
    }
    fn is_low(&mut self) -> Result<bool, Self::Error> {
        Ok(self.pushed)
    }
}

/// Reads an **active-low** input: button wired to ground with an internal
/// pull-up. `true` therefore means "pressed".
///
/// On a read error it returns `false` (idle) rather than panicking: for a
/// button, a failed read is no reason to bring the firmware down.
pub fn read_low_active<P>(pin: &mut P) -> bool
where
    P: InputPin,
{
    pin.is_low().unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs `n` calls in a row and returns only the last event.
    fn feed(b: &mut Debouncer, pressed: bool, n: usize, now_ms: u64) -> PressEvent {
        let mut last = PressEvent::None;
        for _ in 0..n {
            last = b.update(pressed, now_ms);
        }
        last
    }

    #[test]
    fn debounce_ignores_short_glitches() {
        let mut b = Debouncer::new();
        // Four reads, below the threshold of five: nothing should happen.
        for _ in 0..4 {
            assert!(matches!(b.update(true, 0), PressEvent::None));
        }
        // Back to idle: the counter restarts from zero.
        assert!(matches!(b.update(false, 0), PressEvent::None));
        // Five fresh reads are needed to confirm.
        for _ in 0..4 {
            assert!(matches!(b.update(true, 0), PressEvent::None));
        }
        assert!(matches!(b.update(true, 0), PressEvent::Short));
    }

    #[test]
    fn press_emits_short_after_threshold() {
        let mut b = Debouncer::new();
        assert!(matches!(feed(&mut b, true, 5, 0), PressEvent::Short));
    }

    #[test]
    fn long_press_emitted_once_after_delay() {
        let mut b = Debouncer::new();
        feed(&mut b, true, 5, 0);

        // Before the deadline: nothing.
        assert!(matches!(b.update(true, 799), PressEvent::None));

        // On the deadline: Long.
        assert!(matches!(b.update(true, 800), PressEvent::Long));

        // And only once, however long the button is held.
        assert!(matches!(b.update(true, 5_000), PressEvent::None));
    }

    #[test]
    fn release_resets_state_for_next_press() {
        let mut b = Debouncer::new();
        feed(&mut b, true, 5, 0);
        b.update(true, 800); // Long

        // Release (five reads): no event.
        assert!(matches!(feed(&mut b, false, 5, 900), PressEvent::None));

        // Fresh press: Short must come back, then Long.
        assert!(matches!(feed(&mut b, true, 5, 1_000), PressEvent::Short));
        assert!(matches!(b.update(true, 1_800), PressEvent::Long));
    }

    #[test]
    fn fake_pin_is_low_active() {
        let mut pin = FakePin { pushed: true };
        assert!(read_low_active(&mut pin));
        pin.pushed = false;
        assert!(!read_low_active(&mut pin));
    }
}
