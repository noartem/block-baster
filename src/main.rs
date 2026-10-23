// No console window on Windows.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
fn main() {
    block_baster::platform::desktop::run();
}

#[cfg(any(target_arch = "wasm32", target_os = "android"))]
fn main() {}
