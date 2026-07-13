//! `seggwat review local …` — an account-free, on-page review loop backed by
//! plain JSON files under `./.review/`.
//!
//! This is the standalone dev-time mode: no SeggWat account, no project id, no
//! API key. The AI files a review task, the human drops pins on the running app
//! via the review widget, and the AI reads the pins back — all locally. It
//! speaks the same widget HTTP contract as the hosted product, so the same
//! `seggwat-review.js` overlay runs unchanged; annotations live in local JSON
//! instead of a real SeggWat project.
//!
//! The `.review/` files are the single source of truth: every command and every
//! request reloads them, so a running `serve` and one-shot commands
//! (`create`, `comments`, `resolve`) always see each other's writes.

use std::net::SocketAddr;
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;

use anyhow::{Result, anyhow, bail};
use axum::{
    Json, Router,
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    response::Html,
    routing::{get, patch, post},
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tower_http::{cors::CorsLayer, services::ServeDir};

use crate::cli::LocalReviewCommand;
use crate::models::{
    Feedback, FeedbackSource, FeedbackStatus, FeedbackType, PinAnchor, ReviewTask,
    ReviewTaskListItem,
};
use crate::output;

const STORE_DIR: &str = ".review";

// ---------------------------------------------------------------------------
// Store — persisted as pretty JSON in ./.review/{tasks,comments}.json
// ---------------------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize)]
struct Task {
    id: String,
    title: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default = "open_status")]
    status: String, // "open" | "done"
    created_at: String,
    #[serde(default)]
    updated_at: String,
}

fn open_status() -> String {
    "open".into()
}

/// A single annotation. Mirrors a SeggWat `Feedback` row carrying a
/// `review_task_id`: a message plus the overlay's `pin_anchor`, stored verbatim
/// as JSON so any widget-side anchor/shape change needs no server change.
#[derive(Clone, Serialize, Deserialize)]
struct Comment {
    id: String,
    task_id: String,
    message: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    pin_anchor: Value,
    #[serde(default)]
    resolved: bool,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    screenshot: Option<String>, // filename under ./.review/screenshots/
    created_at: String,
}

struct Store {
    dir: PathBuf,
    tasks: Vec<Task>,
    comments: Vec<Comment>,
}

impl Store {
    fn load(dir: PathBuf) -> Self {
        let tasks = read_json(&dir.join("tasks.json")).unwrap_or_default();
        let comments = read_json(&dir.join("comments.json")).unwrap_or_default();
        Store {
            dir,
            tasks,
            comments,
        }
    }

    fn persist(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        write_json(&self.dir.join("tasks.json"), &self.tasks)?;
        write_json(&self.dir.join("comments.json"), &self.comments)?;
        Ok(())
    }

    fn task(&self, id: &str) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == id)
    }

    fn count_comments(&self, task_id: &str, resolved: Option<bool>) -> usize {
        self.comments
            .iter()
            .filter(|c| c.task_id == task_id && resolved.is_none_or(|r| c.resolved == r))
            .count()
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(p: &FsPath) -> Option<T> {
    std::fs::read(p)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
}

fn write_json<T: Serialize>(p: &FsPath, v: &T) -> std::io::Result<()> {
    let s = serde_json::to_string_pretty(v)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(p, s)
}

fn store_dir() -> PathBuf {
    PathBuf::from(STORE_DIR)
}

fn new_id() -> String {
    let mut rng = rand::rng();
    (0..16)
        .map(|_| format!("{:02x}", rng.random::<u8>()))
        .collect()
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// An empty value clears the field (matches the hosted `review update` semantics).
fn clear_or_set(v: &str) -> Option<String> {
    if v.is_empty() {
        None
    } else {
        Some(v.to_string())
    }
}

// ---------------------------------------------------------------------------
// Mapping to the shared CLI display models (identical output to hosted mode)
// ---------------------------------------------------------------------------

fn to_review_task(t: &Task) -> ReviewTask {
    ReviewTask {
        id: t.id.clone(),
        project_id: "local".into(),
        title: t.title.clone(),
        description: t.description.clone(),
        url: t.url.clone(),
        status: t.status.clone(),
        created_at: t.created_at.clone(),
        updated_at: if t.updated_at.is_empty() {
            t.created_at.clone()
        } else {
            t.updated_at.clone()
        },
    }
}

fn to_feedback(c: &Comment) -> Feedback {
    Feedback {
        id: c.id.clone(),
        project_id: "local".into(),
        message: c.message.clone(),
        status: if c.resolved {
            FeedbackStatus::Resolved
        } else {
            FeedbackStatus::New
        },
        feedback_type: FeedbackType::Other,
        source: FeedbackSource::Widget,
        created_at: c.created_at.clone(),
        path: c.path.clone(),
        version: c.version.clone(),
        archived: false,
        submitted_by: None,
        resolution_note: None,
        pin_anchor: serde_json::from_value::<PinAnchor>(c.pin_anchor.clone()).ok(),
    }
}

// ---------------------------------------------------------------------------
// CLI dispatch
// ---------------------------------------------------------------------------

pub async fn execute(cmd: &LocalReviewCommand, json: bool) -> Result<()> {
    match cmd {
        LocalReviewCommand::Serve { port } => serve(*port).await?,
        LocalReviewCommand::List => {
            let store = Store::load(store_dir());
            let items: Vec<ReviewTaskListItem> = store
                .tasks
                .iter()
                .map(|t| ReviewTaskListItem {
                    task: to_review_task(t),
                    feedback_count: store.count_comments(&t.id, None) as u64,
                })
                .collect();
            if json {
                output::print_json(&items)?;
            } else {
                output::print_review_task_table(&items);
            }
        }
        LocalReviewCommand::Get { task_id } => {
            let store = Store::load(store_dir());
            let task = store
                .task(task_id)
                .ok_or_else(|| anyhow!("Task not found: {task_id}"))?;
            let rt = to_review_task(task);
            if json {
                output::print_json(&rt)?;
            } else {
                output::print_review_task_detail(&rt);
            }
        }
        LocalReviewCommand::Comments { task_id } => {
            let store = Store::load(store_dir());
            if store.task(task_id).is_none() {
                bail!("Task not found: {task_id}");
            }
            let comments: Vec<Feedback> = store
                .comments
                .iter()
                .filter(|c| &c.task_id == task_id)
                .map(to_feedback)
                .collect();
            if json {
                output::print_json(&comments)?;
            } else {
                output::print_review_comments_table(&comments);
            }
        }
        LocalReviewCommand::Create {
            title,
            description,
            url,
        } => {
            if title.trim().is_empty() {
                bail!("Title cannot be empty");
            }
            let mut store = Store::load(store_dir());
            let now = now_rfc3339();
            let task = Task {
                id: new_id(),
                title: title.trim().to_string(),
                description: description.clone(),
                url: url.clone(),
                status: "open".into(),
                created_at: now.clone(),
                updated_at: now,
            };
            let rt = to_review_task(&task);
            store.tasks.push(task);
            store.persist()?;
            if json {
                output::print_json(&rt)?;
            } else {
                println!("Review task created: {}", rt.id);
                output::print_review_task_detail(&rt);
                println!();
                println!(
                    "Open your app with ?seggwat-review={} to start pinning.",
                    rt.id
                );
            }
        }
        LocalReviewCommand::Update {
            task_id,
            title,
            description,
            url,
            status,
        } => {
            let status_str = status.as_ref().map(|s| s.to_string());
            if title.is_none() && description.is_none() && url.is_none() && status_str.is_none() {
                bail!(
                    "Nothing to update. Provide at least one of --title, --description, --url, or --status."
                );
            }
            let mut store = Store::load(store_dir());
            let idx = store
                .tasks
                .iter()
                .position(|t| &t.id == task_id)
                .ok_or_else(|| anyhow!("Task not found: {task_id}"))?;
            {
                let t = &mut store.tasks[idx];
                if let Some(v) = title {
                    t.title = v.clone();
                }
                if let Some(v) = description {
                    t.description = clear_or_set(v);
                }
                if let Some(v) = url {
                    t.url = clear_or_set(v);
                }
                if let Some(s) = &status_str {
                    t.status = s.clone();
                }
                t.updated_at = now_rfc3339();
            }
            let rt = to_review_task(&store.tasks[idx]);
            store.persist()?;
            if json {
                output::print_json(&rt)?;
            } else {
                println!("Review task updated: {}", rt.id);
                output::print_review_task_detail(&rt);
            }
        }
        LocalReviewCommand::Delete { task_id } => {
            let mut store = Store::load(store_dir());
            let before = store.tasks.len();
            store.tasks.retain(|t| &t.id != task_id);
            if store.tasks.len() == before {
                bail!("Task not found: {task_id}");
            }
            store.persist()?;
            if json {
                output::print_json(&serde_json::json!({"deleted": task_id}))?;
            } else {
                println!("Review task deleted: {task_id}");
            }
        }
        LocalReviewCommand::Resolve { feedback_id } => set_resolved(feedback_id, true, json)?,
        LocalReviewCommand::Reopen { feedback_id } => set_resolved(feedback_id, false, json)?,
    }
    Ok(())
}

fn set_resolved(feedback_id: &str, resolved: bool, json: bool) -> Result<()> {
    let mut store = Store::load(store_dir());
    let idx = store
        .comments
        .iter()
        .position(|c| c.id == feedback_id)
        .ok_or_else(|| anyhow!("Annotation not found: {feedback_id}"))?;
    store.comments[idx].resolved = resolved;
    store.persist()?;
    if json {
        output::print_json(&to_feedback(&store.comments[idx]))?;
    } else {
        println!(
            "Annotation {}: {}",
            if resolved { "resolved" } else { "reopened" },
            feedback_id
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Local widget-contract server (`seggwat review local serve`)
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct LocalState {
    dir: PathBuf,
    /// Serializes handler read/modify/write cycles against the JSON files.
    lock: Arc<Mutex<()>>,
    index_html: Arc<String>,
}

async fn serve(port: u16) -> Result<()> {
    let dir = store_dir();
    std::fs::create_dir_all(&dir)?;

    let state = LocalState {
        dir: dir.clone(),
        lock: Arc::new(Mutex::new(())),
        index_html: Arc::new(index_page(port)),
    };

    let app = Router::new()
        .route("/", get(index))
        // Widget-facing contract — seggwat-review.js speaks this unchanged.
        .route("/api/v1/p/{key}/review-tasks", get(list_public_tasks))
        .route(
            "/api/v1/p/{key}/review-tasks/{task_id}",
            get(get_public_task),
        )
        .route("/api/v1/feedback/submit", post(submit_feedback))
        .route(
            "/api/v1/feedback/submit-with-screenshot",
            post(submit_feedback_screenshot),
        )
        .route(
            "/api/v1/p/{key}/review-comments/{feedback_id}",
            patch(resolve_comment),
        )
        .nest_service("/screenshots", ServeDir::new(dir.join("screenshots")))
        .with_state(state)
        .layer(CorsLayer::permissive());

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(addr).await?;

    println!("🐐 seggwat review local — on-page review, no account");
    println!("   Storage: ./{STORE_DIR}/   ·   help: http://localhost:{port}/\n");
    println!(
        "Add this to your dev app, create a task, then open it with ?seggwat-review=<task_id>:\n"
    );
    println!("{}", snippet(port));
    println!("\n  seggwat review local create -t \"What to review\"   # then copy the task id");
    println!("  seggwat review local comments <task_id>            # read the pins back");
    println!("\nPress Ctrl+C to stop.");

    axum::serve(listener, app).await?;
    Ok(())
}

async fn index(State(st): State<LocalState>) -> Html<String> {
    Html((*st.index_html).clone())
}

#[derive(Deserialize)]
struct PathQuery {
    #[allow(dead_code)]
    path: Option<String>,
}

/// `GET /api/v1/p/{key}/review-tasks` — open tasks for the in-page picker.
async fn list_public_tasks(
    State(st): State<LocalState>,
    Path(_key): Path<String>,
    Query(_q): Query<PathQuery>,
) -> Json<Value> {
    let _g = st.lock.lock().await;
    let store = Store::load(st.dir.clone());
    let list: Vec<Value> = store
        .tasks
        .iter()
        .filter(|t| t.status == "open")
        .map(|t| {
            json!({
                "task_id": t.id,
                "title": t.title,
                "comment_count": store.count_comments(&t.id, None),
            })
        })
        .collect();
    Json(json!(list))
}

/// `GET /api/v1/p/{key}/review-tasks/{task_id}` — task + its annotations.
async fn get_public_task(
    State(st): State<LocalState>,
    Path((_key, task_id)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    let _g = st.lock.lock().await;
    let store = Store::load(st.dir.clone());
    let task = store.task(&task_id).ok_or(StatusCode::NOT_FOUND)?;
    let comments: Vec<Value> = store
        .comments
        .iter()
        .filter(|c| c.task_id == task_id)
        .map(|c| {
            json!({
                "id": c.id,
                "message": c.message,
                "resolved": c.resolved,
                "pin_anchor": c.pin_anchor,
            })
        })
        .collect();
    Ok(Json(json!({
        "task_id": task.id,
        "title": task.title,
        "status": task.status,
        "comments": comments,
    })))
}

#[derive(Deserialize)]
struct SubmitPayload {
    message: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    review_task_id: Option<String>,
    #[serde(default)]
    pin_anchor: Value,
    #[serde(default)]
    version: Option<String>,
}

fn insert_comment(
    store: &mut Store,
    p: SubmitPayload,
    screenshot: Option<String>,
) -> Result<String, StatusCode> {
    let task_id = p.review_task_id.ok_or(StatusCode::BAD_REQUEST)?;
    if store.task(&task_id).is_none() {
        return Err(StatusCode::NOT_FOUND);
    }
    let id = new_id();
    store.comments.push(Comment {
        id: id.clone(),
        task_id,
        message: p.message,
        path: p.path,
        pin_anchor: p.pin_anchor,
        resolved: false,
        version: p.version,
        screenshot,
        created_at: now_rfc3339(),
    });
    store
        .persist()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(id)
}

/// `POST /api/v1/feedback/submit` — plain JSON pin/annotation.
async fn submit_feedback(
    State(st): State<LocalState>,
    Json(p): Json<SubmitPayload>,
) -> Result<Json<Value>, StatusCode> {
    let _g = st.lock.lock().await;
    let mut store = Store::load(st.dir.clone());
    let id = insert_comment(&mut store, p, None)?;
    Ok(Json(json!({ "id": id, "success": true })))
}

/// `POST /api/v1/feedback/submit-with-screenshot` — multipart: `data` (JSON
/// string) + `screenshot` (image bytes, burned-in annotation from the overlay).
async fn submit_feedback_screenshot(
    State(st): State<LocalState>,
    mut multipart: Multipart,
) -> Result<Json<Value>, StatusCode> {
    let mut payload: Option<SubmitPayload> = None;
    let mut image: Option<Vec<u8>> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
    {
        match field.name() {
            Some("data") => {
                let text = field.text().await.map_err(|_| StatusCode::BAD_REQUEST)?;
                payload = serde_json::from_str(&text).ok();
            }
            Some("screenshot") => {
                let bytes = field.bytes().await.map_err(|_| StatusCode::BAD_REQUEST)?;
                image = Some(bytes.to_vec());
            }
            _ => {}
        }
    }
    let p = payload.ok_or(StatusCode::BAD_REQUEST)?;
    let _g = st.lock.lock().await;
    let mut store = Store::load(st.dir.clone());
    let mut screenshot_name = None;
    if let Some(bytes) = image {
        let shots = store.dir.join("screenshots");
        std::fs::create_dir_all(&shots).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let fname = format!("{}.jpg", new_id());
        if std::fs::write(shots.join(&fname), &bytes).is_ok() {
            screenshot_name = Some(fname);
        }
    }
    let id = insert_comment(&mut store, p, screenshot_name)?;
    Ok(Json(json!({ "id": id, "success": true })))
}

#[derive(Deserialize)]
struct ResolveBody {
    resolved: bool,
}

/// `PATCH /api/v1/p/{key}/review-comments/{feedback_id}` — resolve / reopen.
async fn resolve_comment(
    State(st): State<LocalState>,
    Path((_key, feedback_id)): Path<(String, String)>,
    Json(body): Json<ResolveBody>,
) -> Result<StatusCode, StatusCode> {
    let _g = st.lock.lock().await;
    let mut store = Store::load(st.dir.clone());
    let idx = store
        .comments
        .iter()
        .position(|c| c.id == feedback_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    store.comments[idx].resolved = body.resolved;
    store
        .persist()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::OK)
}

fn snippet(port: u16) -> String {
    format!(
        "<script src=\"https://seggwat.com/static/widgets/v1/seggwat-review.js\"\n        data-project-key=\"local\"\n        data-api-url=\"http://localhost:{port}\"></script>"
    )
}

fn index_page(port: u16) -> String {
    let esc_snippet = snippet(port)
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>seggwat review local</title>
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
  <h1>🐐 Local review loop <span class="muted">— no account</span></h1>
  <p class="muted">Pins are stored in <code>./{STORE_DIR}/</code>. No SeggWat project, no API key.</p>
  <ol>
    <li>Create a task: <code>seggwat review local create -t "Review the new checkout"</code> → copy the id.</li>
    <li>Add this snippet to your dev app:
      <pre>{esc_snippet}</pre>
    </li>
    <li>Open your app with <code>?seggwat-review=&lt;task_id&gt;</code> appended, then drop pins.</li>
    <li>Read them back: <code>seggwat review local comments &lt;task_id&gt;</code> — resolve with <code>seggwat review local resolve &lt;id&gt;</code>.</li>
  </ol>
</body>
</html>"#
    )
}
