//! API server entry point.

use api::config::Config;
use event_store::{InMemoryEventStore, PostgresEventStore};
use tokio::signal;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Waits for a shutdown signal (SIGINT or SIGTERM).
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install SIGINT handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {
            tracing::info!("received SIGINT, starting graceful shutdown");
        }
        () = terminate => {
            tracing::info!("received SIGTERM, starting graceful shutdown");
        }
    }
}

#[tokio::main]
async fn main() {
    // 1. Load configuration
    let config = Config::from_env();

    // 2. Initialize tracing
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log_level)),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!(?config, "loaded configuration");

    // 3. Install Prometheus metrics recorder
    let prometheus_builder = metrics_exporter_prometheus::PrometheusBuilder::new();
    let metrics_handle = prometheus_builder
        .install_recorder()
        .expect("failed to install Prometheus recorder");

    // 4. Create event store and application state (Postgres if DATABASE_URL set, else in-memory)
    let app = if let Some(ref database_url) = config.database_url {
        tracing::info!("connecting to PostgreSQL");
        let store = PostgresEventStore::connect(database_url, config.db_max_connections)
            .await
            .expect("failed to connect to PostgreSQL");
        let (state, processor, _) = api::create_default_state(store);
        processor.run_catch_up().await.expect("catch-up failed");
        api::create_app(state, metrics_handle, processor)
    } else {
        tracing::info!("using in-memory event store");
        let store = InMemoryEventStore::new();
        let (state, processor, _) = api::create_default_state(store);
        processor.run_catch_up().await.expect("catch-up failed");
        api::create_app(state, metrics_handle, processor)
    };

    // 7. Start server
    let addr = config.addr();
    tracing::info!(%addr, "starting API server");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind address");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server error");

    tracing::info!("server shut down gracefully");
}
