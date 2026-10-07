//! The user's system accent color. Native looks follow it instead of an app accent.
//!
//! The value is read when a theme applies. The app re-applies on appearance
//! changes and on window activation, which covers an accent changed in system
//! settings without a platform notification observer.
use gamesync_desktop::native_palette::Accent;

#[cfg(target_os = "macos")]
pub fn read() -> Option<Accent> {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};
    // SAFETY: class methods on NSColor/NSColorSpace return autoreleased objects or nil;
    // each is checked before use and none is retained past this call.
    unsafe {
        let accent: *mut Object = msg_send![class!(NSColor), controlAccentColor];
        if accent.is_null() {
            return None;
        }
        let space: *mut Object = msg_send![class!(NSColorSpace), sRGBColorSpace];
        let rgb: *mut Object = msg_send![accent, colorUsingColorSpace: space];
        if rgb.is_null() {
            return None;
        }
        let channel = |value: f64| (value.clamp(0., 1.) * 255.).round() as u32;
        let r: f64 = msg_send![rgb, redComponent];
        let g: f64 = msg_send![rgb, greenComponent];
        let b: f64 = msg_send![rgb, blueComponent];
        Some(channel(r) << 16 | channel(g) << 8 | channel(b))
    }
}

/// DWM stores the accent as 0xAABBGGRR.
#[cfg(target_os = "windows")]
pub fn read() -> Option<Accent> {
    let abgr = windows_registry::CURRENT_USER
        .open(r"Software\Microsoft\Windows\DWM")
        .and_then(|key| key.get_u32("AccentColor"))
        .ok()?;
    let (r, g, b) = (abgr & 0xff, (abgr >> 8) & 0xff, (abgr >> 16) & 0xff);
    Some(r << 16 | g << 8 | b)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn read() -> Option<Accent> {
    None
}
