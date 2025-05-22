use anyhow::Result;
use axum::{
    Json, Router,
    extract::State,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::info;

pub struct ServerPre {
    start_command: String,
    stop_command: String,
    status_command: String,
}

pub struct Server {
    start_command: String,
    stop_command: String,
    status_command: String,
}

pub fn prepare(start_command: String, stop_command: String, status_command: String) -> ServerPre {
    ServerPre {
        start_command,
        stop_command,
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
            start_command: self.start_command,
            stop_command: self.stop_command,
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
        .route("/start", post(start_post))
        .route("/stop", post(stop_post))
        .with_state(Arc::new(server))
}

async fn root_get() -> Response {
    Json("Hello from command-server!").into_response()
}

async fn status_get(State(server): State<Arc<Server>>) -> Response {
    server.status_command.clone().into_response()
}

async fn start_post(State(server): State<Arc<Server>>) -> Response {
    server.start_command.clone().into_response()
}

async fn stop_post(State(server): State<Arc<Server>>) -> Response {
    server.stop_command.clone().into_response()
}
