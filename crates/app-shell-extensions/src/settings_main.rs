//! The settings page as a process: the same `SettingsSheet` an app links
//! in, served over vox to the host that started it
//! (`extensions/settings.toml`'s `[entry]`). Run by hand it says so and
//! exits.

use app_shell_extensions::architect_extensions::guest::{Guest, GuestError};
use app_shell_extensions::architect_extensions_dioxus::guest::GuestExt as _;

fn main() -> Result<(), GuestError> {
    Guest::extension(&app_shell_extensions::settings_sheet::SettingsSheet).serve()
}
