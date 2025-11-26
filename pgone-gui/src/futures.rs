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
/// 如果当前线程在 Tokio runtime 上下文中，使用 `Handle::current().block_on`
/// 否则创建一个新的 runtime
pub fn block_on_async<F, T>(f: F) -> T
where
    F: Future<Output = T>,
{
    // 尝试获取当前的 runtime handle
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            // 如果当前线程已经在 runtime 中，使用 block_in_place 避免阻塞
            tokio::task::block_in_place(|| {
                handle.block_on(f)
            })
        }
        Err(_) => {
            // 如果没有 runtime，创建一个新的
            tokio::runtime::Runtime::new()
                .expect("Failed to create Tokio runtime")
                .block_on(f)
        }
    }
}