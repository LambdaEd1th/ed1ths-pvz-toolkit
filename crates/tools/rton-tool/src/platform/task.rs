#[cfg(not(target_arch = "wasm32"))]
use futures_timer::Delay;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::JsFuture;

#[cfg(not(target_arch = "wasm32"))]
pub async fn run_cpu_task<T, F>(task: F) -> T
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(task());
    });

    loop {
        match receiver.try_recv() {
            Ok(result) => return result,
            Err(mpsc::TryRecvError::Empty) => Delay::new(Duration::from_millis(16)).await,
            Err(mpsc::TryRecvError::Disconnected) => panic!("background task disconnected"),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn sleep_ms(milliseconds: u64) {
    Delay::new(Duration::from_millis(milliseconds)).await;
}

#[cfg(target_arch = "wasm32")]
pub async fn sleep_ms(milliseconds: u64) {
    timeout(milliseconds as i32).await;
}

#[cfg(target_arch = "wasm32")]
async fn timeout(milliseconds: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        let Some(window) = web_sys::window() else {
            let _ = resolve.call0(&JsValue::UNDEFINED);
            return;
        };
        let callback = Closure::once(move || {
            let _ = resolve.call0(&JsValue::UNDEFINED);
        });
        if let Err(error) = window.set_timeout_with_callback_and_timeout_and_arguments_0(
            callback.as_ref().unchecked_ref(),
            milliseconds,
        ) {
            let _ = reject.call1(&JsValue::UNDEFINED, &error);
            return;
        }
        callback.forget();
    });
    let _ = JsFuture::from(promise).await;
}
