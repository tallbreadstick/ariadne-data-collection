use std::net::SocketAddr;

use axum::body::Body;
use axum::http::{Request, StatusCode, Uri};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::any;
use axum::Router;
use tokio::net::TcpListener;
use tracing::info;

/// Plain-HTTP listener that permanently redirects every request to HTTPS on `https_port`.
pub async fn serve_http_redirect(bind: SocketAddr, https_port: u16) -> anyhow::Result<()> {
    let app = Router::new().fallback(any(move |req: Request<Body>| async move {
        redirect_to_https(req, https_port)
    }));
    let listener = TcpListener::bind(bind).await?;
    info!(%bind, https_port, "HTTP redirect listener started");
    axum::serve(listener, app).await?;
    Ok(())
}

fn redirect_to_https(req: Request<Body>, https_port: u16) -> Response {
    let host = req
        .headers()
        .get(axum::http::header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost");

    let authority = strip_port(host);
    let uri = if https_port == 443 {
        format!("https://{}{}", authority, req.uri().path_and_query().map(|p| p.as_str()).unwrap_or(""))
    } else {
        format!(
            "https://{}:{}{}",
            authority,
            https_port,
            req.uri().path_and_query().map(|p| p.as_str()).unwrap_or("")
        )
    };

    match Uri::try_from(uri) {
        Ok(target) => Redirect::permanent(&target.to_string()).into_response(),
        Err(_) => StatusCode::BAD_REQUEST.into_response(),
    }
}

fn strip_port(host: &str) -> &str {
    host.split(':').next().unwrap_or(host)
}
