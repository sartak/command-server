use anyhow::Result;
use clap::Parser;

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
    dbg!(start_command, stop_command, status_command, port);

    Ok(())
}
