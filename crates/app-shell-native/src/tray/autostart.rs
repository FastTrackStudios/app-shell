//! Start at login, opt-in: the app asks ([`set`]), nothing is automatic.
//!
//! - Linux: an XDG autostart entry, `~/.config/autostart/<id>.desktop`.
//! - macOS: a LaunchAgent the app writes for itself,
//!   `~/Library/LaunchAgents/<id>.plist` (a signed app bundle could use
//!   `SMAppService` instead: see the README).
//! - Windows: a value under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
//!
//! The files' contents are plain functions ([`desktop_entry`],
//! [`launch_agent`], [`run_value`]), tested without touching the system.

use std::io;
use std::path::{Path, PathBuf};

/// What starts at login.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Autostart {
    /// A reverse-DNS id (`studio.fasttrack.drive`): the file's name.
    pub id: String,
    /// The app's name, as a session's settings list it.
    pub name: String,
    /// The program to run.
    pub exec: PathBuf,
    /// Its arguments (a `--background` the app reads to start with only
    /// the tray).
    pub args: Vec<String>,
}

impl Autostart {
    /// This program, with `args`.
    pub fn current(
        id: impl Into<String>,
        name: impl Into<String>,
        args: &[&str],
    ) -> io::Result<Self> {
        Ok(Self {
            id: id.into(),
            name: name.into(),
            exec: std::env::current_exe()?,
            args: args.iter().map(|a| (*a).to_owned()).collect(),
        })
    }
}

/// Quote one argument for a `.desktop` file's `Exec` key (the XDG
/// Desktop Entry spec's rules: double quotes, and `"`, `` ` ``, `$`, `\`
/// escaped; `%` doubled).
fn desktop_quote(arg: &str) -> String {
    let plain = !arg.is_empty()
        && arg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./=:,+@".contains(c));
    if plain {
        return arg.replace('%', "%%");
    }
    let mut s = String::from("\"");
    for c in arg.chars() {
        match c {
            '"' | '`' | '$' | '\\' => {
                s.push('\\');
                s.push(c);
            }
            '%' => s.push_str("%%"),
            _ => s.push(c),
        }
    }
    s.push('"');
    s
}

/// The XDG autostart entry for `a`.
#[must_use]
pub fn desktop_entry(a: &Autostart) -> String {
    let mut exec = desktop_quote(&a.exec.to_string_lossy());
    for arg in &a.args {
        exec.push(' ');
        exec.push_str(&desktop_quote(arg));
    }
    format!(
        "[Desktop Entry]\nType=Application\nName={}\nExec={exec}\nX-GNOME-Autostart-enabled=true\nNoDisplay=false\nTerminal=false\n",
        a.name.replace('\n', " ")
    )
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The LaunchAgent property list for `a` (macOS): run at load, not kept
/// alive (Quit means quit).
#[must_use]
pub fn launch_agent(a: &Autostart) -> String {
    let mut args = format!(
        "\t\t<string>{}</string>\n",
        xml_escape(&a.exec.to_string_lossy())
    );
    for arg in &a.args {
        args.push_str(&format!("\t\t<string>{}</string>\n", xml_escape(arg)));
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n<dict>\n\
         \t<key>Label</key>\n\t<string>{}</string>\n\
         \t<key>ProgramArguments</key>\n\t<array>\n{args}\t</array>\n\
         \t<key>RunAtLoad</key>\n\t<true/>\n\
         \t<key>KeepAlive</key>\n\t<false/>\n\
         \t<key>ProcessType</key>\n\t<string>Interactive</string>\n\
         </dict>\n</plist>\n",
        xml_escape(&a.id)
    )
}

/// The value written under the Run key for `a` (Windows): the program
/// quoted, then its arguments (quoted where they hold a space).
#[must_use]
pub fn run_value(a: &Autostart) -> String {
    let q = |s: &str| {
        if s.contains(' ') || s.is_empty() {
            format!("\"{}\"", s.replace('"', "\\\""))
        } else {
            s.to_owned()
        }
    };
    let mut v = format!("\"{}\"", a.exec.to_string_lossy());
    for arg in &a.args {
        v.push(' ');
        v.push_str(&q(arg));
    }
    v
}

/// Where the autostart file for `id` goes, given the home directory and
/// (Linux) `XDG_CONFIG_HOME`. `None` on Windows (a registry value).
#[must_use]
pub fn file_for(id: &str, home: &Path, xdg_config: Option<&Path>, os: Os) -> Option<PathBuf> {
    match os {
        Os::Linux => Some(
            xdg_config
                .map_or_else(|| home.join(".config"), Path::to_path_buf)
                .join("autostart")
                .join(format!("{id}.desktop")),
        ),
        Os::MacOs => Some(
            home.join("Library")
                .join("LaunchAgents")
                .join(format!("{id}.plist")),
        ),
        Os::Windows => None,
    }
}

/// The platforms autostart knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Linux,
    MacOs,
    Windows,
}

impl Os {
    /// The platform this build runs on.
    #[must_use]
    pub const fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(windows) {
            Self::Windows
        } else {
            Self::Linux
        }
    }
}

fn home() -> io::Result<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no home directory"))
}

fn this_file(id: &str) -> io::Result<Option<PathBuf>> {
    let xdg = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from);
    Ok(file_for(id, &home()?, xdg.as_deref(), Os::current()))
}

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

/// Start `a` at login (`true`), or stop doing so. Writes (or removes) only
/// the app's own file or value.
pub fn set(a: &Autostart, on: bool) -> io::Result<()> {
    match this_file(&a.id)? {
        Some(path) => write_file(&path, a, on, Os::current()),
        None => {
            // Windows: the Run key, through reg.exe (no registry crate).
            let mut cmd = std::process::Command::new("reg");
            if on {
                cmd.args([
                    "add",
                    RUN_KEY,
                    "/v",
                    &a.id,
                    "/t",
                    "REG_SZ",
                    "/d",
                    &run_value(a),
                    "/f",
                ]);
            } else {
                cmd.args(["delete", RUN_KEY, "/v", &a.id, "/f"]);
            }
            let out = cmd.output()?;
            if out.status.success() || !on {
                Ok(())
            } else {
                Err(io::Error::other("reg.exe refused the Run key"))
            }
        }
    }
}

/// Write (or remove) `a`'s autostart file at `path`, as `os` reads it.
fn write_file(path: &Path, a: &Autostart, on: bool, os: Os) -> io::Result<()> {
    if on {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let body = match os {
            Os::MacOs => launch_agent(a),
            _ => desktop_entry(a),
        };
        std::fs::write(path, body)
    } else {
        match std::fs::remove_file(path) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}

/// Whether the app `id` starts at login.
pub fn is_enabled(id: &str) -> io::Result<bool> {
    match this_file(id)? {
        Some(path) => Ok(path.exists()),
        None => Ok(std::process::Command::new("reg")
            .args(["query", RUN_KEY, "/v", id])
            .output()?
            .status
            .success()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drive() -> Autostart {
        Autostart {
            id: "studio.fasttrack.drive".into(),
            name: "Drive".into(),
            exec: PathBuf::from("/opt/Drive Beta/drive"),
            args: vec!["--background".into(), "50% done".into()],
        }
    }

    #[test]
    fn the_desktop_entry_quotes_what_needs_it() {
        let e = desktop_entry(&drive());
        assert!(e.starts_with("[Desktop Entry]\nType=Application\nName=Drive\n"));
        assert!(
            e.contains("Exec=\"/opt/Drive Beta/drive\" --background \"50%% done\"\n"),
            "{e}"
        );
        assert!(e.contains("X-GNOME-Autostart-enabled=true"));
        assert_eq!(desktop_quote("a$b"), "\"a\\$b\"");
    }

    #[test]
    fn the_launch_agent_runs_at_load_and_is_not_kept_alive() {
        let p = launch_agent(&Autostart {
            args: vec!["--background".into(), "<x>".into()],
            ..drive()
        });
        assert!(p.contains("<key>Label</key>\n\t<string>studio.fasttrack.drive</string>"));
        assert!(p.contains("\t\t<string>/opt/Drive Beta/drive</string>\n\t\t<string>--background</string>\n\t\t<string>&lt;x&gt;</string>\n"));
        assert!(p.contains("<key>RunAtLoad</key>\n\t<true/>"));
        assert!(p.contains("<key>KeepAlive</key>\n\t<false/>"));
    }

    #[test]
    fn the_run_value_quotes_the_program_and_spaced_arguments() {
        assert_eq!(
            run_value(&drive()),
            "\"/opt/Drive Beta/drive\" --background \"50% done\""
        );
    }

    #[test]
    fn each_platform_puts_it_in_its_place() {
        let home = Path::new("/home/u");
        assert_eq!(
            file_for("a.b", home, None, Os::Linux),
            Some(PathBuf::from("/home/u/.config/autostart/a.b.desktop"))
        );
        assert_eq!(
            file_for("a.b", home, Some(Path::new("/x/cfg")), Os::Linux),
            Some(PathBuf::from("/x/cfg/autostart/a.b.desktop"))
        );
        assert_eq!(
            file_for("a.b", home, None, Os::MacOs),
            Some(PathBuf::from("/home/u/Library/LaunchAgents/a.b.plist"))
        );
        assert_eq!(file_for("a.b", home, None, Os::Windows), None);
    }

    #[test]
    fn turning_it_on_and_off_writes_and_removes_only_its_own_file() {
        // A scratch directory standing in for the config directory.
        let dir = std::env::temp_dir().join(format!("app-shell-autostart-{}", std::process::id()));
        let path = file_for("t.app", Path::new("/unused"), Some(&dir), Os::Linux).expect("linux");
        let other = dir.join("autostart").join("someone.else.desktop");
        std::fs::create_dir_all(other.parent().expect("dir")).expect("mkdir");
        std::fs::write(&other, "keep").expect("write");
        let a = Autostart {
            id: "t.app".into(),
            ..drive()
        };
        write_file(&path, &a, true, Os::Linux).expect("on");
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            desktop_entry(&a)
        );
        write_file(&path, &a, false, Os::Linux).expect("off");
        assert!(!path.exists());
        write_file(&path, &a, false, Os::Linux).expect("off twice is fine");
        assert_eq!(std::fs::read_to_string(&other).expect("other"), "keep");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
