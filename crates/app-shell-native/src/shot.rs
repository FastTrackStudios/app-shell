//! The app draws its own window to a PNG: no screen capture, no
//! permission, whether the window is covered, minimised or on another
//! desktop. For development tools and agents that need to see the app.
//!
//! The main window's Blitz document is painted again by the CPU
//! rasteriser (`anyrender_vello_cpu`) at the window's pixel size and scale,
//! on the window's thread between two frames; the GPU's framebuffer is not
//! read back (the renderer keeps no copy of a presented frame). What draws
//! itself on the GPU (a custom paint widget with a texture of its own) may
//! be missing from the picture.
//!
//! [`serve`] answers on a local socket, one request a line:
//!
//! ```text
//! shot /tmp/app.png      →  {"path":"/tmp/app.png","width":2560,"height":1600,"scale":2}
//! ```
//!
//! It listens only when the app asks for it (`Config::control`), at
//! `$XDG_RUNTIME_DIR/app-shell/<name>.sock` (the temp dir without one), a
//! socket only this user can open.

use std::cell::RefCell;
use std::io::{BufRead, BufReader, Write as _};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use blitz_dom::BaseDocument;
use dioxus::prelude::*;
use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use futures_util::StreamExt;

/// The main window's document, in the main root's context.
#[derive(Clone)]
pub struct LiveDocument(pub(crate) Rc<RefCell<BaseDocument>>);

/// A shot taken: where it was written and its size in pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct Shot {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
}

/// One request from the socket, and where its answer goes.
pub(crate) struct Job {
    path: PathBuf,
    answer: std::sync::mpsc::Sender<Result<Shot, String>>,
}

/// Where the control socket of the app called `name` is.
#[must_use]
pub fn socket_path(name: &str) -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map_or_else(std::env::temp_dir, PathBuf::from)
        .join("app-shell");
    let file: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    dir.join(format!("{file}.sock"))
}

/// Listen on the control socket (a thread of its own); shots are taken
/// by [`use_shots`] in the main window, which this hands them to.
pub(crate) fn serve(name: &str) -> UnboundedReceiver<Job> {
    let (jobs, rx) = unbounded();
    let path = socket_path(name);
    #[cfg(unix)]
    {
        let _ = std::fs::create_dir_all(path.parent().unwrap_or(Path::new(".")));
        let _ = std::fs::remove_file(&path);
        match std::os::unix::net::UnixListener::bind(&path) {
            Ok(listener) => {
                {
                    use std::os::unix::fs::PermissionsExt as _;
                    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
                }
                tracing::info!(app_shell.control = %path.display(), "control socket");
                std::thread::spawn(move || accept(&listener, &jobs));
            }
            Err(error) => {
                tracing::warn!(app_shell.control = %path.display(), %error, "no control socket");
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (path, jobs);
    }
    rx
}

#[cfg(unix)]
fn accept(listener: &std::os::unix::net::UnixListener, jobs: &UnboundedSender<Job>) {
    for stream in listener.incoming().flatten() {
        let jobs = jobs.clone();
        std::thread::spawn(move || {
            let Ok(read) = stream.try_clone() else { return };
            let mut out = stream;
            for line in BufReader::new(read).lines().map_while(Result::ok) {
                let answer = answer(&line, &jobs);
                if writeln!(out, "{answer}").is_err() {
                    break;
                }
            }
        });
    }
}

/// One line in, one line of JSON out.
fn answer(line: &str, jobs: &UnboundedSender<Job>) -> String {
    let fail = |e: &str| format!("{{\"error\":{}}}", json_string(e));
    let mut words = line.trim().splitn(2, ' ');
    match (words.next(), words.next()) {
        (Some("shot"), path) => {
            let path = match target(path.map(str::trim).filter(|p| !p.is_empty())) {
                Ok(p) => p,
                Err(e) => return fail(&e),
            };
            let (answer, answered) = std::sync::mpsc::channel();
            if jobs.unbounded_send(Job { path, answer }).is_err() {
                return fail("the window is gone");
            }
            match answered.recv_timeout(Duration::from_secs(20)) {
                Ok(Ok(s)) => format!(
                    "{{\"path\":{},\"width\":{},\"height\":{},\"scale\":{}}}",
                    json_string(&s.path.display().to_string()),
                    s.width,
                    s.height,
                    s.scale
                ),
                Ok(Err(e)) => fail(&e),
                Err(_) => fail("the window did not answer"),
            }
        }
        _ => fail("unknown request (try: shot /path/to/file.png)"),
    }
}

fn json_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn target(path: Option<&str>) -> Result<PathBuf, String> {
    let path = path.map_or_else(|| std::env::temp_dir().join("app-shot.png"), PathBuf::from);
    if !path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("png"))
    {
        return Err(format!("{}: a shot's path ends in .png", path.display()));
    }
    std::path::absolute(&path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Take the socket's shots, in the main window (its document is in
/// context, put there by the launcher).
pub(crate) fn use_shots(jobs: Rc<RefCell<Option<UnboundedReceiver<Job>>>>) {
    let live = try_use_context::<LiveDocument>();
    use_future(move || {
        let jobs = jobs.borrow_mut().take();
        let live = live.clone();
        async move {
            let (Some(mut jobs), Some(live)) = (jobs, live) else {
                return;
            };
            while let Some(job) = jobs.next().await {
                let shot = take(&live, &job.path).await;
                let _ = job.answer.send(shot);
            }
        }
    });
}

async fn take(live: &LiveDocument, path: &Path) -> Result<Shot, String> {
    let mut tries = 0_u32;
    let (pixels, width, height, scale) = loop {
        if let Ok(mut doc) = live.0.try_borrow_mut() {
            break paint(&mut doc);
        }
        tries += 1;
        if tries > 200 {
            return Err("the window's document stayed busy".to_owned());
        }
        futures_timer::Delay::new(Duration::from_millis(5)).await;
    };
    write_png(path, &pixels, width, height)?;
    Ok(Shot {
        path: path.to_path_buf(),
        width,
        height,
        scale,
    })
}

fn paint(doc: &mut BaseDocument) -> (Vec<u8>, u32, u32, f64) {
    use anyrender::PaintScene as _;
    // As the last frame resolved it: resolving again here would need the
    // frame's animation clock, which the published blitz-dom keeps private.
    let (width, height) = doc.viewport().window_size;
    let scale = doc.viewport().scale_f64();
    let (width, height) = (width.max(1), height.max(1));
    let pixels = anyrender::render_to_buffer::<anyrender_vello_cpu::VelloCpuImageRenderer, _>(
        |scene| {
            scene.fill(
                peniko::Fill::NonZero,
                peniko::kurbo::Affine::IDENTITY,
                peniko::Color::WHITE,
                Option::<peniko::kurbo::Affine>::None,
                &peniko::kurbo::Rect::new(0.0, 0.0, f64::from(width), f64::from(height)),
            );
            blitz_paint::paint_scene(scene, doc, scale, width, height, 0, 0);
        },
        width,
        height,
    );
    (pixels, width, height, scale)
}

fn write_png(path: &Path, pixels: &[u8], width: u32, height: u32) -> Result<(), String> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(pixels).map_err(|e| e.to_string())?;
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let name = path
        .file_name()
        .map_or_else(|| "shot".into(), |n| n.to_string_lossy().into_owned());
    let temporary = path.with_file_name(format!(".{name}.{}.tmp", std::process::id()));
    std::fs::write(&temporary, &bytes).map_err(|e| format!("{}: {e}", temporary.display()))?;
    std::fs::rename(&temporary, path).map_err(|e| {
        let _ = std::fs::remove_file(&temporary);
        format!("{}: {e}", path.display())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_checked_before_the_window_is_asked() {
        let (jobs, _rx) = unbounded();
        assert!(answer("shot /tmp/x.jpg", &jobs).contains("ends in .png"));
        assert!(answer("hello", &jobs).contains("unknown request"));
        assert_eq!(json_string("a\"b\\c\nd"), "\"a\\\"b\\\\c\\nd\"");
        assert!(socket_path("Studio App").ends_with("app-shell/studio-app.sock"));
    }
}
