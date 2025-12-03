use pgone_util::log;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    log::init_log_from_env()?;
    pgone_gui::run()
}
