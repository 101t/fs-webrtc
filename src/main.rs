mod api;
mod config;
mod freeswitch;

use std::{process::ExitCode, str::FromStr, time::Duration};

use clap::Parser;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

use crate::config::Config;

#[tokio::main]
async fn main() -> ExitCode {
    let mut cfg = Config::parse();
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,tower_http=info,sqlx=warn"));
    let logs = tracing_subscriber::fmt().with_env_filter(filter);
    if cfg.log_json {
        logs.json().init()
    } else {
        logs.init()
    }

    if let Err(e) = cfg.validate() {
        tracing::error!("invalid configuration: {e}");
        return ExitCode::from(2);
    }
    match run(cfg).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!("fatal: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run(cfg: Config) -> Result<(), Box<dyn std::error::Error>> {
    let db = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(
            SqliteConnectOptions::from_str(&cfg.database_url)?
                .create_if_missing(true)
                .foreign_keys(true)
                .journal_mode(SqliteJournalMode::Wal)
                .synchronous(SqliteSynchronous::Normal)
                .busy_timeout(Duration::from_secs(5)),
        )
        .await?;
    sqlx::migrate!().run(&db).await?;
    sqlx::query("DELETE FROM sessions WHERE expires_at < unixepoch()").execute(&db).await?;

    if !cfg.web_dir.join("index.html").exists() {
        tracing::warn!("no web app in {} (set FSW_WEB_DIR); serving the API only", cfg.web_dir.display());
    }
    let listener = TcpListener::bind(cfg.listen).await?;
    tracing::info!("fs-webrtc {} listening on http://{} (API docs at /docs)", env!("CARGO_PKG_VERSION"), cfg.listen);
    let app = api::router(api::AppState::new(cfg, db));
    axum::serve(listener, app).with_graceful_shutdown(shutdown()).await?;
    tracing::info!("shut down cleanly");
    Ok(())
}

async fn shutdown() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    {
        let mut term =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("SIGTERM handler");
        tokio::select! { _ = ctrl_c => {}, _ = term.recv() => {} }
    }
    #[cfg(not(unix))]
    let _ = ctrl_c.await;
}
