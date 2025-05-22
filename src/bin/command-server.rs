use anyhow::Result;
use clap::Parser;
use command_server::server;
use std::net::{Ipv4Addr, TcpListener};
use tokio::{select, signal};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    run_command: String,

    #[arg(long)]
    status_command: String,

    #[arg(long)]
    after_stop_command: Option<String>,

    #[arg(long)]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    let Args {
        run_command,
        status_command,
        after_stop_command,
        port,
    } = Args::parse();

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let listener = TcpListener::bind((Ipv4Addr::new(0, 0, 0, 0), port))?;
    listener.set_nonblocking(true)?;

    let shutdown = shutdown_signal().await;

    let server = server::prepare(run_command, status_command, after_stop_command);
    let server = server.start(listener, shutdown);

    info!(
        monotonic_counter.launched = 1,
        "{} started", "command-server"
    );

    let res = server.await;

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
