#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

#[cfg(target_os = "macos")]
pub fn install_safe_reopen_handler() {
    use objc2::runtime::{AnyClass, AnyObject, Bool, Imp, Sel};
    use objc2::sel;
    use objc2_app_kit::NSApplication;
    use std::sync::Once;

    static INSTALL: Once = Once::new();

    unsafe extern "C-unwind" fn handle_reopen(
        _: &AnyObject,
        _: Sel,
        application: &NSApplication,
        _: Bool,
    ) -> Bool {
        // Dioxus does not consume Tao's `Event::Reopen`, so handle the useful
        // native behavior here without entering Tao's non-unwind-safe Rust
        // callback. This also restores minimized or hidden windows.
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            for window in &*application.windows() {
                if window.isMiniaturized() {
                    window.deminiaturize(None);
                }
                window.makeKeyAndOrderFront(None);
            }
        }));
        Bool::new(true)
    }

    INSTALL.call_once(|| {
        let Some(delegate_class) = AnyClass::get(c"TaoAppDelegateParent") else {
            return;
        };
        let Some(method) =
            delegate_class.instance_method(sel!(applicationShouldHandleReopen:hasVisibleWindows:))
        else {
            return;
        };

        // SAFETY: `handle_reopen` has the exact Objective-C method signature
        // (`id, SEL, NSApplication*, BOOL -> BOOL`) registered by Tao.
        unsafe {
            let handler: unsafe extern "C-unwind" fn(
                &AnyObject,
                Sel,
                &NSApplication,
                Bool,
            ) -> Bool = handle_reopen;
            let handler: Imp = std::mem::transmute(handler);
            method.set_implementation(handler);
        }
    });
}

#[cfg(target_os = "macos")]
/// Makes the Tao-owned AppKit window opaque.
///
/// # Safety
///
/// `ns_window` must be either null or a valid `NSWindow` pointer that remains
/// alive for the duration of this call.
pub unsafe fn make_opaque(ns_window: *mut std::ffi::c_void) {
    use objc2_app_kit::{NSColorSpace, NSWindow};

    let ns_window = ns_window.cast::<NSWindow>();
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
