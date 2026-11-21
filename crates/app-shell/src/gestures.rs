//! The phone layout's decisions and its gestures' arithmetic, as plain
//! functions: which layout a window gets, where the drawer is under a
//! finger and where it settles, where a bottom sheet settles, which way a
//! swipe went, and what the command palette shows for a query. The
//! components (`ui`) call these; the tests hold them without a renderer.

/// Below this width (points) a window is a phone.
pub const PHONE_BELOW: f64 = 600.0;
/// How far in from the left edge a swipe opens the drawer, points.
pub const EDGE_ZONE: f64 = 20.0;
/// How far a press moves before it is a drag, not a tap, points.
pub const DRAG_SLOP: f64 = 8.0;
/// The drawer's widest, points; it is never wider than 85% of the window.
pub const DRAWER_MAX: f64 = 320.0;
/// How far a swipe goes before it pages or closes, points.
pub const SWIPE_DISTANCE: f64 = 60.0;

/// The drawer's width in a window `width` points wide.
#[must_use]
pub fn drawer_width(width: f64) -> f64 {
    (width * 0.85).clamp(1.0, DRAWER_MAX)
}

/// Whether a press at `x` (window points) starts opening the drawer.
#[must_use]
pub fn starts_edge_swipe(x: f64) -> bool {
    (0.0..=EDGE_ZONE).contains(&x)
}

/// How open the drawer is (0 shut, 1 open) with a finger `dx` points from
/// where it pressed, the drawer `width` wide, starting `from` (0 opening
/// from the edge, 1 closing an open one).
#[must_use]
pub fn drawer_progress(from: f64, dx: f64, width: f64) -> f64 {
    (from + dx / width.max(1.0)).clamp(0.0, 1.0)
}

/// Whether the drawer stays open when the finger lets go: past half way,
/// or a decided flick (`dx` past [`SWIPE_DISTANCE`] in either direction
/// wins over the position). A press that never moved past [`DRAG_SLOP`]
/// leaves it as it was (`from`).
#[must_use]
pub fn drawer_settles_open(from: f64, dx: f64, width: f64) -> bool {
    if dx.abs() < DRAG_SLOP {
        return from >= 0.5;
    }
    if dx >= SWIPE_DISTANCE {
        return true;
    }
    if dx <= -SWIPE_DISTANCE {
        return false;
    }
    drawer_progress(from, dx, width) >= 0.5
}

/// How tall a bottom sheet stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Detent {
    /// As tall as its content (a menu, a short form), at most [`Detent::Large`].
    #[default]
    Fit,
    /// Half the window.
    Medium,
    /// Nearly all of it, the page's top edge showing above.
    Large,
}

impl Detent {
    /// The share of the window's height the sheet's top sits at (`Fit`
    /// has no fixed top: `None`).
    #[must_use]
    pub const fn top(self) -> Option<f64> {
        match self {
            Self::Fit => None,
            Self::Medium => Some(0.5),
            Self::Large => Some(0.08),
        }
    }
}

/// Where a sheet goes when the finger lets go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Settle {
    Stay(Detent),
    Dismiss,
}

/// Where a sheet standing at `detent` settles after being dragged `dy`
/// points (down positive) in a window `height` points tall, `detents` the
/// heights it may take. A short drag stays; a long one down dismisses it
/// (a fit sheet past 12% of the window); otherwise the nearest allowed
/// detent wins, a drag past dismissal's halfway point dismissing.
#[must_use]
pub fn sheet_settle(detent: Detent, detents: &[Detent], dy: f64, height: f64) -> Settle {
    let h = height.max(1.0);
    if dy.abs() < DRAG_SLOP {
        return Settle::Stay(detent);
    }
    let Some(top) = detent.top() else {
        // A fit sheet only goes down.
        return if dy > h * 0.12 {
            Settle::Dismiss
        } else {
            Settle::Stay(detent)
        };
    };
    let at = top + dy / h;
    // Dismissal counts as a stop at 90% of the window.
    let mut best = (Settle::Dismiss, (at - 0.9).abs());
    for d in detents {
        if let Some(t) = d.top() {
            let dist = (at - t).abs();
            if dist < best.1 {
                best = (Settle::Stay(*d), dist);
            }
        }
    }
    best.0
}

/// A swipe's direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Swipe {
    Left,
    Right,
    Down,
    Up,
}

/// Which way a finger that moved `(dx, dy)` swiped, if it went far enough
/// ([`SWIPE_DISTANCE`]) and mostly one way (its main axis at least 1.5×
/// the other).
#[must_use]
pub fn swipe_of(dx: f64, dy: f64) -> Option<Swipe> {
    let (ax, ay) = (dx.abs(), dy.abs());
    if ax >= SWIPE_DISTANCE && ax >= ay * 1.5 {
        Some(if dx < 0.0 { Swipe::Left } else { Swipe::Right })
    } else if ay >= SWIPE_DISTANCE && ay >= ax * 1.5 {
        Some(if dy < 0.0 { Swipe::Up } else { Swipe::Down })
    } else {
        None
    }
}

/// How well `query` matches an entry's `label` and `extra` words (higher
/// better; `None`: not at all). Case-insensitive: the whole label starting
/// with it, then a word of it, then anywhere in it, then in the extra
/// words. Every space-separated part of the query must match somewhere.
#[must_use]
pub fn palette_score(query: &str, label: &str, extra: &str) -> Option<u32> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Some(1);
    }
    let l = label.to_lowercase();
    let e = extra.to_lowercase();
    let mut score = 0;
    for part in q.split_whitespace() {
        let s = if l.starts_with(part) {
            40
        } else if l
            .split(|c: char| !c.is_alphanumeric())
            .any(|w| w.starts_with(part))
        {
            30
        } else if l.contains(part) {
            20
        } else if e.contains(part) {
            10
        } else {
            return None;
        };
        score += s;
    }
    Some(score)
}

/// The order a palette shows `entries` (label, extra) in for `query`: the
/// matches, best first, ties in their given order.
#[must_use]
pub fn palette_order(query: &str, entries: &[(&str, &str)]) -> Vec<usize> {
    let mut hits: Vec<(usize, u32)> = entries
        .iter()
        .enumerate()
        .filter_map(|(i, (l, e))| palette_score(query, l, e).map(|s| (i, s)))
        .collect();
    hits.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    hits.into_iter().map(|(i, _)| i).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_drawer_is_most_of_a_phone_and_at_most_320() {
        assert!((drawer_width(390.0) - 320.0).abs() < 1e-9);
        assert!((drawer_width(320.0) - 272.0).abs() < 1e-9);
        assert!((drawer_width(1200.0) - 320.0).abs() < 1e-9);
    }

    #[test]
    fn only_a_press_at_the_left_edge_opens_the_drawer() {
        assert!(starts_edge_swipe(0.0) && starts_edge_swipe(20.0));
        assert!(!starts_edge_swipe(21.0) && !starts_edge_swipe(-1.0));
    }

    #[test]
    fn the_drawer_follows_the_finger_within_its_width() {
        assert!((drawer_progress(0.0, 150.0, 300.0) - 0.5).abs() < 1e-9);
        assert!((drawer_progress(0.0, 900.0, 300.0) - 1.0).abs() < 1e-9);
        assert!((drawer_progress(1.0, -75.0, 300.0) - 0.75).abs() < 1e-9);
        assert!((drawer_progress(1.0, 50.0, 300.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn the_drawer_settles_by_flick_then_by_half_way_and_a_tap_changes_nothing() {
        assert!(drawer_settles_open(0.0, 70.0, 300.0), "a flick open");
        assert!(!drawer_settles_open(1.0, -70.0, 300.0), "a flick shut");
        assert!(!drawer_settles_open(0.0, 40.0, 300.0), "not far enough");
        assert!(
            drawer_settles_open(1.0, -40.0, 300.0),
            "not far enough to close"
        );
        assert!(
            drawer_settles_open(1.0, 3.0, 300.0),
            "a tap in an open drawer"
        );
        assert!(!drawer_settles_open(0.0, 3.0, 300.0));
    }

    #[test]
    fn a_sheet_settles_on_the_nearest_detent_or_goes() {
        let both = [Detent::Medium, Detent::Large];
        let h = 800.0;
        assert_eq!(
            sheet_settle(Detent::Medium, &both, 4.0, h),
            Settle::Stay(Detent::Medium)
        );
        assert_eq!(
            sheet_settle(Detent::Medium, &both, -300.0, h),
            Settle::Stay(Detent::Large)
        );
        assert_eq!(
            sheet_settle(Detent::Medium, &both, -100.0, h),
            Settle::Stay(Detent::Medium)
        );
        assert_eq!(
            sheet_settle(Detent::Medium, &both, 200.0, h),
            Settle::Dismiss
        );
        assert_eq!(
            sheet_settle(Detent::Medium, &both, 100.0, h),
            Settle::Stay(Detent::Medium)
        );
        assert_eq!(
            sheet_settle(Detent::Large, &both, 250.0, h),
            Settle::Stay(Detent::Medium)
        );
        assert_eq!(
            sheet_settle(Detent::Large, &[Detent::Large], 250.0, h),
            Settle::Stay(Detent::Large)
        );
        assert_eq!(
            sheet_settle(Detent::Large, &[Detent::Large], 450.0, h),
            Settle::Dismiss
        );
    }

    #[test]
    fn a_fit_sheet_only_goes_down_and_away() {
        assert_eq!(
            sheet_settle(Detent::Fit, &[], -300.0, 800.0),
            Settle::Stay(Detent::Fit)
        );
        assert_eq!(
            sheet_settle(Detent::Fit, &[], 60.0, 800.0),
            Settle::Stay(Detent::Fit)
        );
        assert_eq!(
            sheet_settle(Detent::Fit, &[], 120.0, 800.0),
            Settle::Dismiss
        );
    }

    #[test]
    fn a_swipe_needs_distance_and_one_main_direction() {
        assert_eq!(swipe_of(-80.0, 10.0), Some(Swipe::Left));
        assert_eq!(swipe_of(80.0, -20.0), Some(Swipe::Right));
        assert_eq!(swipe_of(10.0, 120.0), Some(Swipe::Down));
        assert_eq!(swipe_of(5.0, -90.0), Some(Swipe::Up));
        assert_eq!(swipe_of(40.0, 0.0), None, "too short");
        assert_eq!(swipe_of(80.0, 70.0), None, "diagonal");
    }

    #[test]
    fn the_palette_puts_prefixes_first_and_needs_every_word() {
        let entries = [
            ("Upload files", "new add"),
            ("New folder", "create"),
            ("Show tags", "labels"),
            ("Settings", ""),
            ("Reupload", ""),
        ];
        assert_eq!(palette_order("", &entries), vec![0, 1, 2, 3, 4]);
        assert_eq!(palette_order("up", &entries), vec![0, 4]);
        assert_eq!(palette_order("new", &entries), vec![1, 0]);
        assert_eq!(palette_order("label", &entries), vec![2]);
        assert_eq!(palette_order("show tag", &entries), vec![2]);
        assert_eq!(palette_order("show nothing", &entries), Vec::<usize>::new());
        assert_eq!(palette_order("SET", &entries), vec![3]);
    }
}
