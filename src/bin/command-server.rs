use anyhow::Result;
use clap::Parser;
use tracing::{error, info};
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

fn main() -> Result<()> {
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

    info!(
        monotonic_counter.launched = 1,
        "{} started", "command-server"
    );

    let res = Ok(());

    if let Err(e) = &res {
        error!("main shutting down with error: {e:?}");
    } else {
        info!("main gracefully shut down");
    }

    res
}
