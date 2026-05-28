use std::future::Future;
use std::time::Duration;

//pub use maybe_sync::{BoxFuture, MaybeSend, MaybeSync};

#[cfg(not(target_arch = "wasm32"))]
pub fn spawn(f: impl Future<Output = ()> + Send + 'static) {
    tokio::spawn(f);
}

#[cfg(target_arch = "wasm32")]
pub fn spawn(f: impl Future<Output = ()> + 'static) {
    wasm_bindgen_futures::spawn_local(f);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn sleep(dur: Duration) -> impl Future<Output = ()> {
    tokio::time::sleep(dur)
}

#[cfg(target_arch = "wasm32")]
pub fn sleep(dur: Duration) -> impl Future<Output = ()> {
    gloo_timers::future::TimeoutFuture::new(dur.as_millis() as u32)
}

/// Execute async code in a synchronous context.
/// If the current thread is within a Tokio runtime context, uses `Handle::current().block_on`.
/// Otherwise, creates a new runtime.
pub fn block_on_async<F, T>(f: F) -> T
where
    F: Future<Output = T>,
{
    // Try to get the current runtime handle
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            // If the current thread is already in a runtime, use block_in_place to avoid blocking
            tokio::task::block_in_place(|| handle.block_on(f))
        }
        Err(_) => {
            // If there is no runtime, create a new one
            tokio::runtime::Runtime::new()
                .expect("Failed to create Tokio runtime")
                .block_on(f)
        }
    }
}
