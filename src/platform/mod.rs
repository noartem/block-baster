//! Platform services: clock, storage, haptics and the window/event loop shells.

/// A 32-bit seed from the clock (like `Date.now() >>> 0`).
pub fn time_seed() -> u32 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now() as u64 as u32
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u32)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub mod native;

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
pub mod desktop;

#[cfg(target_os = "android")]
pub mod android;

#[cfg(any(target_os = "android", test))]
pub mod legacy;

#[cfg(target_arch = "wasm32")]
pub mod web;
