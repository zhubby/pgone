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

/// 在同步上下文中执行异步代码
/// 使用 `tokio::task::block_in_place` 和 `tokio::runtime::Handle::current().block_on` 来执行异步 Future
pub fn block_on_async<F, T>(f: F) -> T
where
    F: Future<Output = T>,
{
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(f)
    })
}