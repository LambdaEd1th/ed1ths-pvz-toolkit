#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

#[cfg(target_os = "macos")]
pub fn make_opaque(window: &tao::window::Window) {
    use objc2_app_kit::{NSColorSpace, NSWindow};
    use tao::platform::macos::WindowExtMacOS;

    let ns_window = window.ns_window().cast::<NSWindow>();
    if ns_window.is_null() {
        return;
    }

    // Tao owns this NSWindow while the WGPU surface and transparent WebView share it.
    unsafe {
        let window = &*ns_window;
        window.setOpaque(true);
        window.setColorSpace(Some(&NSColorSpace::sRGBColorSpace()));
    }
}
