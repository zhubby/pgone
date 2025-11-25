use tracing_subscriber::EnvFilter;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // 在主线程启动 GUI（macOS 需要主线程运行窗口循环）
    // GUI 会阻塞主线程，但 tokio runtime 的其他工作线程可以继续运行异步任务
    // 注意：必须在主线程直接调用，不能使用 spawn_blocking
    pgone_gui::run()
}
