//! A clock for what a view animates itself: the time in seconds, a wait
//! of about one display frame, and whether motion is reduced.
//!
//! A view that eases a value (a pane sliding over another, a highlight
//! fading) loops: read [`now`], set the value for that time, await
//! [`next_frame`], until it is there. With [`reduced_motion`] on it sets
//! the end at once instead. Drive what Blitz draws cheaply: a
//! `transform`, an `opacity`, a clipped box's `width`.

use std::cell::Cell;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use web_time::Instant;

/// About one display frame at 60 Hz.
pub const FRAME: Duration = Duration::from_micros(16_667);

thread_local! {
    static ORIGIN: Cell<Option<Instant>> = const { Cell::new(None) };
}

/// Seconds since this thread first asked: a clock to measure an
/// animation's progress against.
#[must_use]
pub fn now() -> f64 {
    ORIGIN.with(|origin| {
        let start = origin.get().unwrap_or_else(|| {
            let start = Instant::now();
            origin.set(Some(start));
            start
        });
        start.elapsed().as_secs_f64()
    })
}

/// Resolves after about one display frame ([`FRAME`]).
pub async fn next_frame() {
    futures_timer::Delay::new(FRAME).await;
}

/// 0: not set (motion plays); 1: reduced; 2: set to play.
static REDUCED: AtomicU8 = AtomicU8::new(0);

/// Whether animations should jump to their end: what
/// [`set_reduced_motion`] said, else off.
#[must_use]
pub fn reduced_motion() -> bool {
    REDUCED.load(Ordering::Relaxed) == 1
}

/// Reduces motion (`Some(true)`), lets it play (`Some(false)`), or goes
/// back to the default, playing (`None`). For the person's setting, and
/// for tests that want every animation at its end.
pub fn set_reduced_motion(reduced: Option<bool>) {
    let v = match reduced {
        None => 0,
        Some(true) => 1,
        Some(false) => 2,
    };
    REDUCED.store(v, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clock_moves_forward() {
        let a = now();
        std::thread::sleep(Duration::from_millis(2));
        assert!(now() > a);
    }

    #[test]
    fn reduced_motion_is_off_until_set() {
        set_reduced_motion(None);
        assert!(!reduced_motion());
        set_reduced_motion(Some(true));
        assert!(reduced_motion());
        set_reduced_motion(Some(false));
        assert!(!reduced_motion());
        set_reduced_motion(None);
    }
}
