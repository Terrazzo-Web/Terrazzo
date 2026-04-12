#![cfg(feature = "server")]

use std::env::set_current_dir;
use std::iter::once;

use clap::Parser as _;
use terrazzo::axum;
use terrazzo::axum::Router;
use terrazzo::axum::extract::Path;
use terrazzo::axum::routing::get;
use terrazzo::http::header::AUTHORIZATION;
use terrazzo::static_assets;
use tower_http::sensitive_headers::SetSensitiveRequestHeadersLayer;
use tower_http::trace::TraceLayer;
use tracing::Level;
use tracing::enabled;
use tracing::info;

use crate::api;
use crate::assets;

const PORT: u16 = if cfg!(debug_assertions) { 3001 } else { 3000 };

#[derive(clap::Parser)]
struct Args {
    #[arg(short = 'p', long = "port", default_value_t = PORT)]
    port: u16,

    #[arg(long = "set_current_port")]
    set_current_port: Option<String>,
}

pub async fn run_server() {
    set_current_dir(std::env::var("HOME").expect("HOME")).expect("set_current_dir");
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(true)
        .with_line_number(true)
        .with_file(true)
        .with_target(false)
        .init();

    assets::install_assets();
    let router = Router::new()
        .route("/", get(|| static_assets::get("index.html")))
        .nest_service("/api", api::server::route())
        .route(
            "/static/{*file}",
            get(|Path(path): Path<String>| static_assets::get(&path)),
        );
    let router = router.layer(SetSensitiveRequestHeadersLayer::new(once(AUTHORIZATION)));
    let router = if enabled!(Level::TRACE) {
        router.layer(TraceLayer::new_for_http())
    } else {
        router
    };
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", args.port))
        .await
        .unwrap();
    let local_addr = listener.local_addr().unwrap();
    info!("listening on {}", local_addr);
    if let Some(set_current_port) = &args.set_current_port {
        std::fs::write(set_current_port, local_addr.port().to_string())
            .expect("write port to file");
    }
    axum::serve(listener, router).await.unwrap();
}
