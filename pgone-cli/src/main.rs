#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    pgone_cli::run().await
}
