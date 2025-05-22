use anyhow::Result;
use clap::Parser;
use tokio::{select, signal};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    start_command: String,

    #[arg(long)]
    stop_command: String,

    #[arg(long)]
    status_command: String,

    #[arg(long)]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    let Args {
        start_command,
        stop_command,
        status_command,
        port,
    } = Args::parse();

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let shutdown = shutdown_signal().await;

    info!(
        monotonic_counter.launched = 1,
        "{} started", "command-server"
    );

    let res = select!(
        _ = async {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        } => Ok(()),
        _ = async {
            shutdown.cancelled().await;
        } => Ok(()),
    )
    .map(|_| ());

    if let Err(e) = &res {
        error!("main shutting down with error: {e:?}");
    } else {
        info!("main gracefully shut down");
    }

    res
}

async fn shutdown_signal() -> CancellationToken {
    let token = CancellationToken::new();

    {
        let token = token.clone();
        tokio::spawn(async move {
            let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate()).unwrap();
            select!(
                _ = signal::ctrl_c() => info!(monotonic_counter.shutdown = 1, method = "ctrl-c", "Got interrupt signal, shutting down"),
                _ = sigterm.recv() => info!(monotonic_counter.shutdown = 1, method = "sigterm", "Got sigterm, shutting down"),
            );

            token.cancel();

            select!(
                _ = signal::ctrl_c() => {},
                _ = sigterm.recv() => {},
            );
            warn!("Received multiple shutdown signals, exiting now");
            std::process::exit(1);
        });
    }

    token
}
