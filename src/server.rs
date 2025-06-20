use anyhow::Result;
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Serialize;
use std::sync::Arc;
use tokio::{
    process::{Child, Command},
    sync::Mutex,
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

pub struct ServerPre {
    run_command: String,
    status_command: String,
    before_stop_command: Option<String>,
    after_stop_command: Option<String>,
}

pub struct Server {
    run_command: String,
    status_command: String,
    before_stop_command: Option<String>,
    after_stop_command: Option<String>,

    child: Mutex<Option<Child>>,
}

pub fn prepare(
    run_command: String,
    status_command: String,
    before_stop_command: Option<String>,
    after_stop_command: Option<String>,
) -> ServerPre {
    ServerPre {
        run_command,
        status_command,
        before_stop_command,
        after_stop_command,
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
            before_stop_command: self.before_stop_command,
            after_stop_command: self.after_stop_command,
            child: Mutex::new(None),
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

#[derive(Serialize)]
struct StatusResponse {
    running: bool,
    output: String,
}

async fn status_get(State(server): State<Arc<Server>>) -> Response {
    let lock = server.child.lock().await;
    let running = lock.is_some();

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

    let output = match String::from_utf8(output.stdout) {
        Ok(o) => o,
        Err(e) => {
            error!(
                "Failed to convert output of status command '{}' to UTF-8: {:?}",
                server.status_command, e
            );
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    drop(lock);

    Json(StatusResponse { running, output }).into_response()
}

async fn run_post(State(server): State<Arc<Server>>) -> Response {
    let mut lock = server.child.lock().await;

    if lock.is_some() {
        warn!("Cannot start command, already running");
        return StatusCode::CONFLICT.into_response();
    }

    let child = Command::new("sh")
        .arg("-c")
        .arg(&server.run_command)
        .spawn();

    let child = match child {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to run command '{}': {:?}", server.run_command, e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    *lock = Some(child);

    StatusCode::OK.into_response()
}

async fn stop_post(State(server): State<Arc<Server>>) -> Response {
    let mut lock = server.child.lock().await;

    match *lock {
        Some(ref mut child) => {
            if let Some(command) = server.before_stop_command.as_ref() {
                let output = Command::new("sh").arg("-c").arg(command).output().await;

                let output = match output {
                    Ok(o) => o,
                    Err(e) => {
                        error!("Failed to run before-stop command '{}': {:?}", command, e);
                        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                    }
                };

                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                if !output.status.success() {
                    error!(
                        "before-stop command '{}' failed {}: {}",
                        command, output.status, stderr
                    );
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }

                if !stderr.is_empty() {
                    warn!(
                        "before-stop command '{}' produced stderr: {}",
                        command, stderr
                    );
                }

                if !stdout.is_empty() {
                    warn!(
                        "before-stop command '{}' produced stdout: {}",
                        command, stdout
                    );
                }
            }

            if let Err(e) = child.kill().await {
                error!("Failed to kill command: {:?}", e);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
            if let Err(e) = child.wait().await {
                error!("Failed to wait for command: {:?}", e);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }

            *lock = None;

            if let Some(command) = server.after_stop_command.as_ref() {
                let output = Command::new("sh").arg("-c").arg(command).output().await;

                let output = match output {
                    Ok(o) => o,
                    Err(e) => {
                        error!("Failed to run after-stop command '{}': {:?}", command, e);
                        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                    }
                };

                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                if !output.status.success() {
                    error!(
                        "after-stop command '{}' failed {}: {}",
                        command, output.status, stderr
                    );
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }

                if !stderr.is_empty() {
                    warn!(
                        "after-stop command '{}' produced stderr: {}",
                        command, stderr
                    );
                }

                if !stdout.is_empty() {
                    warn!(
                        "after-stop command '{}' produced stdout: {}",
                        command, stdout
                    );
                }
            }

            drop(lock);

            StatusCode::OK.into_response()
        }
        None => {
            warn!("Cannot stop command, not running");
            StatusCode::CONFLICT.into_response()
        }
    }
}
