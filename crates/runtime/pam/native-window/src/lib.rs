#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

const STAGE_MANAGER_LANE_RATIO: f64 = 0.125;
const MIN_STAGE_MANAGER_LANE: f64 = 180.0;
const MAX_STAGE_MANAGER_LANE: f64 = 220.0;
const RIGHT_EDGE_GAP: f64 = 16.0;
const MIN_WINDOW_FRAME_WIDTH: f64 = 720.0;

#[derive(Clone, Copy, Debug, PartialEq)]
struct WindowFrame {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl WindowFrame {
    fn is_valid(self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
            && self.width > 0.0
            && self.height > 0.0
    }
}

fn fit_startup_frame(window: WindowFrame, visible: WindowFrame) -> WindowFrame {
    if !window.is_valid() || !visible.is_valid() {
        return window;
    }

    // macOS does not include the Stage Manager shelf in NSScreen.visibleFrame.
    // Keep a proportional lane at the leading edge, but give it up on very
    // small displays so the application's minimum width remains usable.
    let desired_lane = (visible.width * STAGE_MANAGER_LANE_RATIO)
        .clamp(MIN_STAGE_MANAGER_LANE, MAX_STAGE_MANAGER_LANE);
    let maximum_lane = (visible.width - RIGHT_EDGE_GAP - MIN_WINDOW_FRAME_WIDTH).max(0.0);
    let leading_lane = desired_lane.min(maximum_lane);
    let usable_width = (visible.width - leading_lane - RIGHT_EDGE_GAP).max(1.0);

    let width = window.width.min(usable_width);
    let height = window.height.min(visible.height);
    let centered_x = visible.x + (visible.width - width) / 2.0;
    let minimum_x = visible.x + leading_lane;
    let maximum_x = visible.x + visible.width - RIGHT_EDGE_GAP - width;
    let x = centered_x.clamp(minimum_x, maximum_x.max(minimum_x));
    let y = visible.y + (visible.height - height) / 2.0;

    WindowFrame {
        x,
        y,
        width,
        height,
    }
}

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
            let windows = application.windows();
            for index in 0..windows.count() {
                let window = windows.objectAtIndex(index);
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

#[cfg(target_os = "macos")]
/// Fits the startup window into the current screen while leaving room for the
/// Stage Manager shelf when the restored window would otherwise cover it.
///
/// Dioxus creates the native window hidden and invokes its window callback
/// before the WebView is shown, so applying the final frame here avoids a
/// visible resize and prevents WindowManager from observing an oversized
/// initial frame.
///
/// # Safety
///
/// `ns_window` must be either null or a valid `NSWindow` pointer that remains
/// alive for the duration of this call. The function must run on the main
/// thread.
pub unsafe fn fit_window_to_visible_frame(ns_window: *mut std::ffi::c_void) {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSScreen, NSWindow};

    let ns_window = ns_window.cast::<NSWindow>();
    if ns_window.is_null() {
        return;
    }

    // SAFETY: The caller guarantees that the Tao-owned NSWindow is live for
    // the duration of this callback.
    let window = unsafe { &*ns_window };
    let screen = window
        .screen()
        .or_else(|| MainThreadMarker::new().and_then(NSScreen::mainScreen));
    let Some(screen) = screen else {
        return;
    };

    let current = window.frame();
    let visible = screen.visibleFrame();
    let fitted = fit_startup_frame(
        WindowFrame {
            x: current.origin.x,
            y: current.origin.y,
            width: current.size.width,
            height: current.size.height,
        },
        WindowFrame {
            x: visible.origin.x,
            y: visible.origin.y,
            width: visible.size.width,
            height: visible.size.height,
        },
    );

    let mut frame = current;
    frame.origin.x = fitted.x;
    frame.origin.y = fitted.y;
    frame.size.width = fitted.width;
    frame.size.height = fitted.height;
    window.setFrame_display(frame, false);
}

#[cfg(test)]
mod tests {
    use super::{WindowFrame, fit_startup_frame};

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 0.01,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn keeps_a_small_window_centered() {
        let fitted = fit_startup_frame(
            WindowFrame {
                x: 0.0,
                y: 0.0,
                width: 958.0,
                height: 661.0,
            },
            WindowFrame {
                x: 0.0,
                y: 24.0,
                width: 1512.0,
                height: 958.0,
            },
        );

        assert_close(fitted.x, 277.0);
        assert_close(fitted.y, 172.5);
        assert_close(fitted.width, 958.0);
        assert_close(fitted.height, 661.0);
    }

    #[test]
    fn leaves_room_on_a_macbook_air_display() {
        let fitted = fit_startup_frame(
            WindowFrame {
                x: 15.0,
                y: 0.0,
                width: 1440.0,
                height: 932.0,
            },
            WindowFrame {
                x: 0.0,
                y: 24.0,
                width: 1470.0,
                height: 932.0,
            },
        );

        assert_close(fitted.x, 183.75);
        assert_close(fitted.width, 1270.25);
        assert_close(fitted.height, 932.0);
    }

    #[test]
    fn shifts_without_shrinking_on_a_wider_macbook_display() {
        let fitted = fit_startup_frame(
            WindowFrame {
                x: 144.0,
                y: 0.0,
                width: 1440.0,
                height: 932.0,
            },
            WindowFrame {
                x: 0.0,
                y: 24.0,
                width: 1728.0,
                height: 1050.0,
            },
        );

        assert_close(fitted.x, 216.0);
        assert_close(fitted.width, 1440.0);
        assert_close(fitted.y, 83.0);
    }

    #[test]
    fn leaves_large_display_windows_unchanged_and_centered() {
        let fitted = fit_startup_frame(
            WindowFrame {
                x: 1200.0,
                y: 0.0,
                width: 1440.0,
                height: 932.0,
            },
            WindowFrame {
                x: 0.0,
                y: 90.0,
                width: 3840.0,
                height: 2040.0,
            },
        );

        assert_close(fitted.x, 1200.0);
        assert_close(fitted.y, 644.0);
        assert_close(fitted.width, 1440.0);
        assert_close(fitted.height, 932.0);
    }

    #[test]
    fn clamps_a_window_restored_from_a_larger_display() {
        let fitted = fit_startup_frame(
            WindowFrame {
                x: 0.0,
                y: 0.0,
                width: 3000.0,
                height: 1800.0,
            },
            WindowFrame {
                x: 0.0,
                y: 24.0,
                width: 1512.0,
                height: 958.0,
            },
        );

        assert_close(fitted.x, 189.0);
        assert_close(fitted.y, 24.0);
        assert_close(fitted.width, 1307.0);
        assert_close(fitted.height, 958.0);
    }

    #[test]
    fn collapses_the_reserved_lane_on_a_tiny_display() {
        let fitted = fit_startup_frame(
            WindowFrame {
                x: 0.0,
                y: 0.0,
                width: 1440.0,
                height: 900.0,
            },
            WindowFrame {
                x: 0.0,
                y: 0.0,
                width: 700.0,
                height: 500.0,
            },
        );

        assert_close(fitted.x, 0.0);
        assert_close(fitted.y, 0.0);
        assert_close(fitted.width, 684.0);
        assert_close(fitted.height, 500.0);
    }

    #[test]
    fn respects_an_external_display_origin() {
        let fitted = fit_startup_frame(
            WindowFrame {
                x: -1500.0,
                y: 100.0,
                width: 1440.0,
                height: 900.0,
            },
            WindowFrame {
                x: -1728.0,
                y: 80.0,
                width: 1728.0,
                height: 1050.0,
            },
        );

        assert_close(fitted.x, -1512.0);
        assert_close(fitted.y, 155.0);
        assert_close(fitted.width, 1440.0);
        assert_close(fitted.height, 900.0);
    }
}
