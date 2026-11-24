mod api;
mod config;
mod error;
mod git;
mod model;
mod pwa;
mod router;
mod runner;
mod state;
mod store;

use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use axum::Router;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::Config;
use crate::state::AppState;
use crate::store::Store;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_env("ONI_LOG")
                .unwrap_or_else(|_| EnvFilter::new("oni_daemon=info,tower_http=info")),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env()?;

    tokio::fs::create_dir_all(&config.data_dir)
        .await
        .with_context(|| format!("cannot create {}", config.data_dir.display()))?;
    tokio::fs::create_dir_all(&config.logs_dir).await?;
    tokio::fs::create_dir_all(&config.worktrees_dir).await?;

    let store = Store::load(config.data_dir.join("oni.json")).await?;
    let interrupted = store.mark_interrupted_tasks().await?;
    if interrupted > 0 {
        tracing::warn!(count = interrupted, "marked interrupted tasks as failed");
    }

    let (events, _) = broadcast::channel(8192);
    let bind_address = format!("{}:{}", config.host, config.port);
    let pwa_dir = config.pwa_dir.clone();
    let data_dir = config.data_dir.clone();
    let auth_enabled = config.token.is_some();
    let app_version = env!("CARGO_PKG_VERSION");

    let state = AppState {
        store,
        events,
        runners: Default::default(),
        config: Arc::new(config),
        started: Instant::now(),
    };

    let api = api::router(state.clone()).layer(axum::middleware::from_fn_with_state(
        state.clone(),
        api::auth,
    ));

    let router = Router::new().nest("/api", api);

    let app = match &pwa_dir {
        Some(dir) => {
            let index = dir.join("index.html");
            if !dir.exists() {
                tracing::warn!(
                    path = %dir.display(),
                    "PWA directory not found; API still works"
                );
            }
            router.fallback_service(ServeDir::new(dir).fallback(ServeFile::new(index)))
        }
        None => router.fallback_service(Router::new().fallback(pwa::serve)),
    };

    let app = app.layer(TraceLayer::new_for_http());

    let listener = TcpListener::bind(&bind_address)
        .await
        .with_context(|| format!("cannot bind {bind_address}"))?;

    tracing::info!("ONI daemon v{app_version}");
    tracing::info!("  api:  http://{bind_address}/api");
    tracing::info!(
        "  pwa:  {} (http://{bind_address})",
        pwa_dir
            .as_ref()
            .map(|dir| format!("disk {}", dir.display()))
            .unwrap_or_else(|| "embedded".to_string())
    );
    tracing::info!("  data: {}", data_dir.display());
    tracing::info!(
        "  auth: {}",
        if auth_enabled {
            "token required"
        } else {
            "disabled (localhost only)"
        }
    );
    tracing::info!(
        "  router: {}",
        state
            .config
            .juniperus_url
            .as_deref()
            .map(|url| format!("juniperus at {url} (fallback enabled)"))
            .unwrap_or_else(|| "deterministic fallback".to_string())
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error")?;

    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutting down");
}
