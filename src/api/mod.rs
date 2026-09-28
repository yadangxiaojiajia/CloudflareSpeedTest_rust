//! HTTP 接口层：只做参数接收与状态下发，不含任何测速逻辑

use crate::config::{self, SpeedConfig};
use crate::task::Runner;
use crate::upload;
use crate::utils::resolve;
use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{header, StatusCode};
use axum::response::sse::{self, KeepAlive, Sse};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;
use std::convert::Infallible;
use std::path::PathBuf;
use futures::stream::{self, Stream};
use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct AppState {
    pub runner: Arc<Runner>,
    pub data_dir: PathBuf,
}

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/app.js", get(js))
        .route("/style.css", get(css))
        .route("/api/config", get(get_config).post(save_config))
        .route("/api/status", get(get_status))
        .route("/api/start", post(start))
        .route("/api/cancel", post(cancel))
        .route("/api/events", get(events))
        .route("/api/results", get(get_results))
        .route("/api/logs", get(get_logs))
        .route("/api/download", get(download))
        .route("/api/system", get(system))
        .route("/api/report-config", get(get_report_config).post(save_report_config))
        .route("/api/upload/worker", post(upload_worker))
        .route("/api/upload/github", post(upload_github))
        .with_state(state)
}

// ---------- 静态资源（编译期内嵌） ----------

async fn index() -> Html<&'static str> {
    Html(crate::web::index_html())
}

async fn js() -> Response {
    Response::builder()
        .header(header::CONTENT_TYPE, "application/javascript; charset=utf-8")
        .body(Body::from(crate::web::app_js()))
        .unwrap()
}

async fn css() -> Response {
    Response::builder()
        .header(header::CONTENT_TYPE, "text/css; charset=utf-8")
        .body(Body::from(crate::web::style_css()))
        .unwrap()
}

// ---------- handlers ----------

async fn get_config(State(s): State<AppState>) -> Json<serde_json::Value> {
    let cfg = config::load(&s.data_dir);
    Json(json!({ "ok": true, "config": cfg }))
}

async fn save_config(
    State(s): State<AppState>,
    Json(cfg): Json<SpeedConfig>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    config::save(&s.data_dir, &cfg).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(json!({ "ok": true })))
}

async fn get_status(State(s): State<AppState>) -> Json<serde_json::Value> {
    Json(json!({
        "running": s.runner.running(),
        "count": s.runner.results().len(),
    }))
}

async fn start(
    State(s): State<AppState>,
    Json(cfg): Json<SpeedConfig>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // 记住这次参数，下次打开界面自动回填
    let _ = config::save(&s.data_dir, &cfg);
    match s.runner.start(cfg) {
        Ok(true) => Ok(Json(json!({ "ok": true }))),
        Ok(false) => Err((StatusCode::CONFLICT, "已有测速任务在运行".into())),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

async fn cancel(State(s): State<AppState>) -> Json<serde_json::Value> {
    s.runner.cancel();
    Json(json!({ "ok": true }))
}

async fn get_results(State(s): State<AppState>) -> Json<serde_json::Value> {
    Json(json!({ "results": s.runner.results() }))
}

async fn get_logs(State(s): State<AppState>) -> Json<serde_json::Value> {
    Json(json!({ "logs": s.runner.logs() }))
}

async fn system(State(s): State<AppState>) -> Json<serde_json::Value> {
    let cfg = config::load(&s.data_dir);
    Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "data_dir": s.data_dir,
        "output": "result.csv",
        "defaults": SpeedConfig::default(),
        "cli_args": cfg.to_cli_args().join(" "),
    }))
}

// ---------- 上报配置与上报 ----------

async fn get_report_config(State(s): State<AppState>) -> Json<serde_json::Value> {
    let rc = upload::load(&s.data_dir);
    Json(json!({
        "ok": true,
        "worker_domain": rc.worker_domain,
        "uuid": rc.uuid,
        "github_repo": rc.github_repo,
        "github_path": rc.github_path,
        "has_github_token": !rc.github_token.is_empty(),
    }))
}

async fn save_report_config(
    State(s): State<AppState>,
    Json(v): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let mut rc = upload::load(&s.data_dir);
    apply_str(&mut rc.worker_domain, v.get("worker_domain"));
    apply_str(&mut rc.uuid, v.get("uuid"));
    apply_str(&mut rc.github_repo, v.get("github_repo"));
    apply_str(&mut rc.github_path, v.get("github_path"));
    // Token 留空 = 沿用已保存的
    if let Some(t) = v.get("github_token").and_then(|x| x.as_str()) {
        if !t.trim().is_empty() {
            rc.github_token = t.trim().to_string();
        }
    }
    upload::save(&s.data_dir, &rc)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e }))))?;
    Ok(Json(json!({ "ok": true, "has_github_token": !rc.github_token.is_empty() })))
}

fn apply_str(field: &mut String, v: Option<&serde_json::Value>) {
    if let Some(x) = v.and_then(|x| x.as_str()) {
        *field = x.trim().to_string();
    }
}

async fn upload_worker(
    State(s): State<AppState>,
    Json(v): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let mut rc = upload::load(&s.data_dir);
    apply_str(&mut rc.worker_domain, v.get("worker_domain"));
    apply_str(&mut rc.uuid, v.get("uuid"));
    let limit = v.get("limit").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
    let clear = v.get("clear").and_then(|x| x.as_bool()).unwrap_or(true);

    let rows = s.runner.results();
    let n = upload::upload_worker(&rc, &rows, limit, clear)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, Json(json!({ "error": e }))))?;
    let _ = upload::save(&s.data_dir, &rc);
    Ok(Json(json!({ "ok": true, "count": n })))
}

async fn upload_github(
    State(s): State<AppState>,
    Json(v): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let mut rc = upload::load(&s.data_dir);
    apply_str(&mut rc.github_repo, v.get("repo"));
    apply_str(&mut rc.github_path, v.get("path"));
    let token = v
        .get("token")
        .and_then(|x| x.as_str())
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .unwrap_or_else(|| rc.github_token.clone());
    let limit = v.get("limit").and_then(|x| x.as_u64()).unwrap_or(0) as usize;

    let rows = s.runner.results();
    let n = upload::upload_github(&rc, &token, &rows, limit)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, Json(json!({ "error": e }))))?;
    if !token.is_empty() {
        rc.github_token = token;
    }
    let _ = upload::save(&s.data_dir, &rc);
    Ok(Json(json!({ "ok": true, "count": n })))
}

async fn download(Query(q): Query<std::collections::HashMap<String, String>>) -> Response {
    let name = q.get("file").cloned().unwrap_or_else(|| "result.csv".into());
    let path = resolve(&name);
    match tokio::fs::read_to_string(&path).await {
        Ok(content) => Response::builder()
            .header(header::CONTENT_TYPE, "text/csv; charset=utf-8")
            .header(
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", name),
            )
            .body(content.into())
            .unwrap(),
        Err(_) => (StatusCode::NOT_FOUND, "文件不存在，请先完成一次测速").into_response(),
    }
}

/// SSE 事件流：20 秒心跳，事件 JSON 与参考 Web UI 字段一致
async fn events(
    State(s): State<AppState>,
) -> Sse<impl Stream<Item = Result<sse::Event, Infallible>>> {
    let rx = s.runner.subscribe();
    let stream = stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(e) => {
                    let data = serde_json::to_string(&e).unwrap_or_default();
                    return Some((Ok(sse::Event::default().data(data)), rx));
                }
                // 订阅者处理慢导致丢事件：跳过继续等下一条，不关闭连接
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::new().interval(std::time::Duration::from_secs(20)))
}
