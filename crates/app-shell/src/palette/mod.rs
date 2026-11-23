//! The command palette's engine: [`Source`]s give [`Item`]s, the
//! [`Engine`] asks them all (or the one a `kind:` prefix names) and puts
//! what they gave in order, with [`fuzzy`] matching, [`rank`]ing and
//! [`Recents`]; [`Openers`] are open-anything's (⌘O). No UI: the shell's
//! palette (`ui::Shell::open_palette`, Mod+K) ranks its entries and the
//! sources and openers in its [`crate::ui::PaletteSources`] through it.
//!
//! From Architect's `architect-palette` (MIT, by the same authors), moved
//! here as it was.

// The engine's boxed closures, moved as they were.
#![allow(clippy::type_complexity)]

mod fuzzy;
mod item;
mod open;
mod query;
mod rank;
mod recent;
mod source;

pub use fuzzy::{Match, fuzzy};
pub use item::{Effect, Item};
pub use open::{Found, OpenIn, OpenItem, Opener, Openers};
pub use query::{Query, Scope};
pub use rank::{Ranked, rank, rank_open, score_command, score_open};
pub use recent::Recents;
pub use source::{Engine, FnSource, Ranking, Source};
