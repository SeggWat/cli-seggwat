//! `seggwat review serve` — a local reverse proxy for the on-page review widget.
//!
//! Runs a tiny HTTP server on localhost that forwards the review widget's API
//! calls to SeggWat, injecting an allowed `Origin` header so the public widget
//! endpoints accept them. This lets you annotate your local dev app against a
//! real SeggWat project **without** adding your dev origin to the project's
//! allowed-origins list — the proxy speaks with an origin the project already
//! trusts.
//!
//! Usage: point the widget's `data-api-url` at this proxy:
//! ```html
//! <script src="https://seggwat.com/static/widgets/v1/seggwat-review.js"
//!         data-project-key="<key>"
//!         data-api-url="http://localhost:7777"></script>
//! ```
//! then open your app with `?seggwat-review=<task_id>`.

use anyhow::Result;
use axum::{
    Router,
    body::{Body, Bytes},
    extract::State,
    http::{HeaderMap, Method, StatusCode, Uri, header::CONTENT_TYPE},
    response::{Html, IntoResponse, Response},
    routing::{any, get},
};
use tower_http::cors::CorsLayer;

use crate::client::SeggwatClient;

#[derive(Clone)]
struct ProxyState {
    http: reqwest::Client,
    /// Upstream API base, e.g. `https://seggwat.com/api/v1`.
    upstream_base: String,
    /// Origin header injected on every forwarded request.
    forward_origin: String,
    index_html: String,
}

pub async fn run(
    client: &SeggwatClient,
    project_id: &str,
    port: u16,
    origin: Option<String>,
) -> Result<()> {
    // Authenticated lookup for the project's key + trusted origins.
    let project = client.get_project(project_id).await?;
    let forward_origin = origin
        .or_else(|| project.allowed_origins.first().cloned())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Project has no allowed origins to forward. Pass --origin <url> with a value the project accepts."
            )
        })?;

    let snippet = format!(
        "<script src=\"https://seggwat.com/static/widgets/v1/seggwat-review.js\"\n        data-project-key=\"{}\"\n        data-api-url=\"http://localhost:{}\"></script>",
        project.key, port
    );

    let state = ProxyState {
        http: reqwest::Client::new(),
        upstream_base: client.base_url().to_string(),
        forward_origin: forward_origin.clone(),
        index_html: index_page(&project.name, &snippet),
    };

    let app = build_router(state);

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    let root = client.base_url().trim_end_matches("/api/v1");
    println!("🐐 seggwat review serve — project \"{}\"", project.name);
    println!("   Forwarding widget API → {root}  (Origin: {forward_origin})");
    println!("   Snippet + help: http://localhost:{port}/\n");
    println!("Add this to your dev app, then open it with ?seggwat-review=<task_id>:\n");
    println!("{snippet}\n");
    println!("Press Ctrl+C to stop.");

    axum::serve(listener, app).await?;
    Ok(())
}

fn build_router(state: ProxyState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/api/v1/{*path}", any(proxy))
        .with_state(state)
        .layer(CorsLayer::permissive())
}

async fn index(State(st): State<ProxyState>) -> Html<String> {
    Html(st.index_html)
}

/// Transparent reverse proxy: forward the request to the upstream API with a
/// trusted `Origin`, so the public widget endpoints (origin-gated) accept it.
async fn proxy(
    State(st): State<ProxyState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let path = uri.path().strip_prefix("/api/v1/").unwrap_or("");
    let query = uri.query().map(|q| format!("?{q}")).unwrap_or_default();
    let url = format!("{}/{}{}", st.upstream_base, path, query);

    let mut req = st
        .http
        .request(method, &url)
        .header("Origin", &st.forward_origin);
    if let Some(ct) = headers.get(CONTENT_TYPE) {
        req = req.header(CONTENT_TYPE, ct);
    }
    if !body.is_empty() {
        req = req.body(body);
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status();
            let ct = resp.headers().get(reqwest::header::CONTENT_TYPE).cloned();
            let bytes = resp.bytes().await.unwrap_or_default();
            let mut builder = Response::builder().status(status.as_u16());
            if let Some(ct) = ct {
                builder = builder.header(CONTENT_TYPE, ct);
            }
            builder
                .body(Body::from(bytes))
                .unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response())
        }
        Err(e) => (StatusCode::BAD_GATEWAY, format!("proxy error: {e}")).into_response(),
    }
}

fn index_page(project_name: &str, snippet: &str) -> String {
    let esc_snippet = snippet
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>seggwat review serve</title>
<style>
  body {{ font: 15px/1.6 -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
          max-width: 720px; margin: 48px auto; padding: 0 20px; color: #1f2937; }}
  h1 {{ font-size: 22px; }} code, pre {{ font-family: ui-monospace, Menlo, monospace; }}
  pre {{ background: #0f172a; color: #e2e8f0; padding: 16px; border-radius: 10px; overflow-x: auto; }}
  ol {{ padding-left: 20px; }} li {{ margin: 8px 0; }}
  .muted {{ color: #6b7280; }}
</style>
</head>
<body>
  <h1>🐐 Review proxy for <em>{project_name}</em></h1>
  <p class="muted">Annotate your local dev app against this SeggWat project. Pins go straight to the project — no allowed-origins setup needed.</p>
  <ol>
    <li>Add this snippet to your dev app:
      <pre>{esc_snippet}</pre>
    </li>
    <li>Create a review task (dashboard, <code>seggwat review create</code>, or MCP) and copy its task id.</li>
    <li>Open your app with <code>?seggwat-review=&lt;task_id&gt;</code> appended, then drop pins.</li>
    <li>Read them back with <code>seggwat review comments &lt;project&gt; &lt;task&gt;</code>.</li>
  </ol>
</body>
</html>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Json, extract::Request, routing::any as any_route};
    use serde_json::json;
    use tokio::net::TcpListener;

    /// Fake SeggWat upstream that echoes back what the proxy forwarded, so we
    /// can assert the proxy injected a trusted Origin and passed everything
    /// through unchanged.
    async fn echo(req: Request) -> Json<serde_json::Value> {
        let method = req.method().to_string();
        let path = req.uri().path().to_string();
        let query = req.uri().query().unwrap_or("").to_string();
        let origin = req
            .headers()
            .get("origin")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let body = axum::body::to_bytes(req.into_body(), 64 * 1024)
            .await
            .unwrap_or_default();
        let body = String::from_utf8_lossy(&body).to_string();
        Json(
            json!({ "method": method, "path": path, "query": query, "origin": origin, "body": body }),
        )
    }

    async fn spawn(app: Router) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn proxy_injects_origin_and_forwards_everything() {
        // Fake upstream, mounted at the same `/{*path}` shape the proxy targets.
        let upstream = spawn(Router::new().route("/{*path}", any_route(echo))).await;

        let state = ProxyState {
            http: reqwest::Client::new(),
            upstream_base: upstream,
            forward_origin: "https://trusted.example".to_string(),
            index_html: String::new(),
        };
        let proxy_url = spawn(build_router(state)).await;

        let resp = reqwest::Client::new()
            .post(format!("{proxy_url}/api/v1/feedback/submit?path=/pricing"))
            .header("origin", "http://localhost:3000") // the untrusted dev origin
            .json(&json!({ "message": "hi", "review_task_id": "abc" }))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);
        let echoed: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(echoed["method"], "POST");
        // `/api/v1/` is stripped before forwarding to the upstream base.
        assert_eq!(echoed["path"], "/feedback/submit");
        assert_eq!(echoed["query"], "path=/pricing");
        // The dev origin is replaced with the project-trusted origin.
        assert_eq!(echoed["origin"], "https://trusted.example");
        assert!(echoed["body"].as_str().unwrap().contains("review_task_id"));
    }
}
