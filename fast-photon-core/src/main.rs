#[tokio::main]
async fn main() -> anyhow::Result<()> {
    fast_photon_core::run_cli().await
}
