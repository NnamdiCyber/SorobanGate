use clap::Parser;
use sorobangate::config;

#[derive(Parser)]
#[command(
    name = "sorobangate",
    version,
    about = "High-performance Soroban RPC gateway & load balancer"
)]
struct Cli {
    #[arg(short, long, default_value = "sorobangate.toml")]
    config: String,

    #[arg(long, default_value_t = false)]
    skip_initial_health_check: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let config = config::Config::load(&cli.config)?;
    config.validate()?;

    let log_filter = format!("sorobangate={}", config.server.log_level.as_str());

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_new(&log_filter)
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(true);

    match config.server.log_format {
        config::LogFormat::Json => subscriber.json().init(),
        config::LogFormat::Pretty => subscriber.pretty().init(),
    }

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        pools = %config.pools.len(),
        "SorobanGate starting"
    );

    sorobangate::server::serve(config, cli.skip_initial_health_check).await?;

    Ok(())
}
