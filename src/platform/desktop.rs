//! The desktop app (Linux, Windows, macOS).

use std::path::PathBuf;

use winit::dpi::LogicalSize;
use winit::event_loop::EventLoop;
use winit::window::{Icon, Window};

use super::native::{Host, Shell};
use crate::app::{App, Platform, Strength};
use crate::storage::{FileStore, KeyValue};

struct Desktop;

impl Platform for Desktop {
    fn can_vibrate(&self) -> bool {
        false
    }

    fn vibrate(&mut self, _strength: Strength, _intensity: f32) {}

    fn is_app(&self) -> bool {
        true
    }

    fn exit(&mut self) {
        // The shell saves on the way out; closing the process is enough.
        std::process::exit(0);
    }

    fn open_url(&mut self, url: &str) {
        let opener = if cfg!(target_os = "windows") {
            std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn()
        } else if cfg!(target_os = "macos") {
            std::process::Command::new("open").arg(url).spawn()
        } else {
            std::process::Command::new("xdg-open").arg(url).spawn()
        };
        if let Err(e) = opener {
            eprintln!("could not open {url}: {e}");
        }
    }
}

struct DesktopHost {
    capture: Option<PathBuf>,
}

impl Host for DesktopHost {
    fn capture(&self) -> Option<PathBuf> {
        self.capture.clone()
    }
}

/// Where saves live: the platform's per-user data directory.
fn data_dir() -> PathBuf {
    let env = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    if cfg!(target_os = "windows") {
        env("APPDATA").unwrap_or_else(|| PathBuf::from(".")).join("Block Baster")
    } else if cfg!(target_os = "macos") {
        env("HOME").unwrap_or_else(|| PathBuf::from(".")).join("Library/Application Support/Block Baster")
    } else {
        env("XDG_DATA_HOME").or_else(|| env("HOME").map(|h| h.join(".local/share"))).unwrap_or_else(|| PathBuf::from(".")).join("block-baster")
    }
}

fn icon() -> Option<Icon> {
    let decoder = png::Decoder::new(std::io::Cursor::new(include_bytes!("../../public/pwa-192.png").as_slice()));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    if info.color_type != png::ColorType::Rgba {
        return None;
    }
    Icon::from_rgba(buf[..info.buffer_size()].to_vec(), info.width, info.height).ok()
}

const USAGE: &str = "Block Baster — the relaxing block puzzle

Usage: block-baster [options]
  --seed <n>        start a reproducible game (the saved game is kept)
  --reset           clear the best score, saved game and settings
  --data <dir>      keep saves in this directory
  --size <w>x<h>    window size in logical pixels (default 430x860)
  --screenshot <f>  render until the screen settles, save it as PNG and quit";

/// Runs the desktop app with command-line options.
pub fn run() {
    let mut args = std::env::args().skip(1);
    let mut seed = None;
    let mut reset = false;
    let mut dir = data_dir();
    let mut size = (430.0, 860.0);
    let mut capture = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--seed" => seed = args.next().and_then(|v| v.parse().ok()),
            "--reset" => reset = true,
            "--data" => dir = args.next().map(PathBuf::from).unwrap_or(dir),
            "--size" => {
                if let Some((w, h)) = args.next().as_deref().and_then(|v| v.split_once('x')) {
                    size = (w.parse().unwrap_or(size.0), h.parse().unwrap_or(size.1));
                }
            }
            "--screenshot" => capture = args.next().map(PathBuf::from),
            "-h" | "--help" => {
                println!("{USAGE}");
                return;
            }
            other => {
                eprintln!("unknown option {other}\n\n{USAGE}");
                std::process::exit(2);
            }
        }
    }
    let mut store = FileStore::open(dir.join("storage.json"));
    if reset {
        store.clear();
    }
    let event_loop = EventLoop::new().expect("event loop");
    let app = App::new(Box::new(store), Box::new(Desktop), seed, 0.0);
    let attributes = Window::default_attributes()
        .with_title("Block Baster")
        .with_inner_size(LogicalSize::new(size.0, size.1))
        .with_min_inner_size(LogicalSize::new(320.0, 480.0))
        .with_window_icon(icon());
    Shell::new(app, DesktopHost { capture }, attributes).run(event_loop);
}
