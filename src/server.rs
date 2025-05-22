use anyhow::Result;
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use std::sync::Arc;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

pub struct ServerPre {
    run_command: String,
    status_command: String,
}

pub struct Server {
    run_command: String,
    status_command: String,
}

pub fn prepare(run_command: String, status_command: String) -> ServerPre {
    ServerPre {
        run_command,
        status_command,
    }
}

impl ServerPre {
    pub async fn start(
        self,
        listener: std::net::TcpListener,
        shutdown: CancellationToken,
    ) -> Result<()> {
        let server = Server {
            run_command: self.run_command,
            status_command: self.status_command,
        };

        let address = listener.local_addr()?;
        let listener = tokio::net::TcpListener::from_std(listener)?;
        let listener = axum::serve(listener, router(server).into_make_service())
            .with_graceful_shutdown(shutdown.cancelled_owned());

        info!("Listening on {address}");
        listener.await?;
        Ok(())
    }
}

fn router(server: Server) -> Router {
    Router::new()
        .route("/", get(root_get))
        .route("/status", get(status_get))
        .route("/run", post(run_post))
        .route("/stop", post(stop_post))
        .with_state(Arc::new(server))
}

async fn root_get() -> Response {
    Json("Hello from command-server!").into_response()
}

async fn status_get(State(server): State<Arc<Server>>) -> Response {
    let output = Command::new("sh")
        .arg("-c")
        .arg(&server.status_command)
        .output()
        .await;

    let output = match output {
        Ok(o) => o,
        Err(e) => {
            error!(
                "Failed to run status command '{}': {:?}",
                server.status_command, e
            );
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!(
            "Status command '{}' failed {}: {}",
            server.status_command, output.status, stderr
        );
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    output.stdout.into_response()
}

async fn run_post(State(server): State<Arc<Server>>) -> Response {
    server.run_command.clone().into_response()
}

async fn stop_post(State(server): State<Arc<Server>>) -> Response {
    ().into_response()
}
