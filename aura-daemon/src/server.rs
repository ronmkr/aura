use crate::router::create_router;
use crate::types::AppState;
use axum_server::Handle;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::info;

pub async fn start_server(
    state: Arc<AppState>,
    bind_address: std::net::IpAddr,
    rpc_port: u16,
    allowed_origins: Vec<String>,
    tls_config: Option<(PathBuf, PathBuf)>,
    shutdown_timeout_secs: u64,
    mut shutdown_rx: tokio::sync::mpsc::Receiver<()>,
) -> Result<(), Box<dyn std::error::Error>> {
    let origins = allowed_origins.clone();
    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::AllowOrigin::predicate(
            move |origin, _parts| {
                let origin_bytes = origin.as_bytes();
                origins
                    .iter()
                    .any(|allowed| origin_bytes.starts_with(allowed.as_bytes()))
            },
        ))
        .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
        .allow_headers(tower_http::cors::Any);

    let app = create_router(state).layer(cors);
    let addr = format!("{}:{}", bind_address, rpc_port);

    if let Some((cert_path, key_path)) = tls_config {
        let rustls_config =
            axum_server::tls_rustls::RustlsConfig::from_pem_file(cert_path, key_path).await?;
        let handle = Handle::new();
        let shutdown_handle = handle.clone();

        tokio::spawn(async move {
            let _ = shutdown_rx.recv().await;
            info!("RPC server stopping (HTTPS)");
            shutdown_handle
                .graceful_shutdown(Some(std::time::Duration::from_secs(shutdown_timeout_secs)));
        });

        info!("RPC Server listening (HTTPS) on https://{}", addr);
        axum_server::bind_rustls(addr.parse::<std::net::SocketAddr>()?, rustls_config)
            .handle(handle)
            .serve(app.into_make_service())
            .await?;
    } else {
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        info!("RPC Server listening on http://{}", addr);

        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.recv().await;
                info!("RPC server stopped");
            })
            .await?;
    }

    Ok(())
}
