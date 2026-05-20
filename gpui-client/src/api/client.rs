use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::Serialize;
use tokio::sync::{mpsc, oneshot};
use url::Url;

use super::types::{
    CommentsJsonResponse, DiffCommentThread, DiffResponse, GeneratedStatusResponse,
    RevisionsResponse,
};

#[derive(Debug, Clone)]
pub enum WatchEvent {
    FilesChanged,
    CommentsChanged { version: u64 },
    Other(String),
}

/// HTTP client targeting a running difit server.
///
/// All methods return a `oneshot::Receiver` so callers (GPUI views) can poll
/// the result inside `cx.spawn(...)` without ever entering the tokio runtime
/// themselves — the I/O is driven on `runtime` and the channel ferries the
/// answer back.
pub struct ApiClient {
    base_url: Url,
    runtime: tokio::runtime::Handle,
    http: Client,
}

impl ApiClient {
    pub fn new(base_url: Url, runtime: tokio::runtime::Handle) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("difit-gpui/0.1")
            .build()
            .expect("failed to build reqwest client");
        Self {
            base_url,
            runtime,
            http,
        }
    }

    pub fn fetch_diff(&self, params: DiffQuery) -> oneshot::Receiver<Result<DiffResponse>> {
        self.get_json("/api/diff", &params)
    }

    pub fn fetch_revisions(&self) -> oneshot::Receiver<Result<RevisionsResponse>> {
        self.get_json("/api/revisions", &EmptyQuery)
    }

    pub fn fetch_comments(
        &self,
        query: &CommentSelectionQuery,
    ) -> oneshot::Receiver<Result<CommentsJsonResponse>> {
        self.get_json("/api/comments-json", query)
    }

    /// Replace the comment session's thread list. The server broadcasts a
    /// `commentsChanged` event on success so the caller doesn't need to
    /// refetch directly.
    pub fn post_comments(
        &self,
        query: &CommentSelectionQuery,
        threads: Vec<DiffCommentThread>,
    ) -> oneshot::Receiver<Result<()>> {
        #[derive(Serialize)]
        struct Payload {
            threads: Vec<DiffCommentThread>,
        }
        self.post_json("/api/comments", query, &Payload { threads })
    }

    /// Ask the server whether a file looks generated (lockfile / minified /
    /// has a `@generated` marker / etc.). Used for auto-collapsing.
    pub fn fetch_generated_status(
        &self,
        path: String,
        git_ref: String,
    ) -> oneshot::Receiver<Result<GeneratedStatusResponse>> {
        #[derive(Serialize)]
        struct Query {
            #[serde(rename = "ref")]
            r: String,
        }
        // The server's path is /api/generated-status/<filepath>. We piggy-back
        // on `get_json` by encoding the path into the URL ourselves.
        let (tx, rx) = oneshot::channel();
        let url = match self.base_url.join(&format!("/api/generated-status/{path}")) {
            Ok(u) => u,
            Err(e) => {
                let _ = tx.send(Err(anyhow!(e).context("invalid generated-status url")));
                return rx;
            }
        };
        let mut url = url;
        let qs = match serde_urlencoded::to_string(Query { r: git_ref }) {
            Ok(s) => s,
            Err(e) => {
                let _ = tx.send(Err(anyhow!(e).context("encoding query")));
                return rx;
            }
        };
        if !qs.is_empty() {
            url.set_query(Some(&qs));
        }
        let http = self.http.clone();
        self.runtime.spawn(async move {
            let result = async {
                let resp = http
                    .get(url.clone())
                    .send()
                    .await
                    .with_context(|| format!("GET {url}"))?;
                if !resp.status().is_success() {
                    return Err(anyhow!("GET {url} failed: HTTP {}", resp.status().as_u16()));
                }
                let parsed: GeneratedStatusResponse = resp
                    .json()
                    .await
                    .with_context(|| format!("decoding {url}"))?;
                Ok(parsed)
            }
            .await;
            let _ = tx.send(result);
        });
        rx
    }

    /// Fetch raw file content at a given ref. Used to expand context lines
    /// around diff chunks.
    pub fn fetch_blob(&self, path: String, git_ref: String) -> oneshot::Receiver<Result<Vec<u8>>> {
        let (tx, rx) = oneshot::channel();
        let url = match self.base_url.join(&format!("/api/blob/{path}")) {
            Ok(u) => u,
            Err(e) => {
                let _ = tx.send(Err(anyhow!(e).context("invalid blob url")));
                return rx;
            }
        };
        let mut url = url;
        url.set_query(Some(&format!("ref={git_ref}")));
        let http = self.http.clone();
        self.runtime.spawn(async move {
            let result = async {
                let resp = http
                    .get(url.clone())
                    .send()
                    .await
                    .with_context(|| format!("GET {url}"))?;
                if !resp.status().is_success() {
                    return Err(anyhow!("GET {url} failed: HTTP {}", resp.status().as_u16()));
                }
                Ok(resp.bytes().await?.to_vec())
            }
            .await;
            let _ = tx.send(result);
        });
        rx
    }

    /// Ask the server to launch the configured editor at `file_path:line`.
    pub fn open_in_editor(
        &self,
        file_path: String,
        line: Option<u32>,
    ) -> oneshot::Receiver<Result<()>> {
        #[derive(Serialize)]
        struct Payload {
            #[serde(rename = "filePath")]
            file_path: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            line: Option<u32>,
        }
        self.post_json("/api/open-in-editor", &EmptyQuery, &Payload { file_path, line })
    }

    /// Subscribe to `/api/watch`. Returns an mpsc receiver yielding parsed
    /// events; the underlying task auto-reconnects on transient failure.
    pub fn watch_stream(&self) -> mpsc::UnboundedReceiver<WatchEvent> {
        let (tx, rx) = mpsc::unbounded_channel();
        let http = self.http.clone();
        let url = self.base_url.join("/api/watch");
        self.runtime.spawn(async move {
            let url = match url {
                Ok(u) => u,
                Err(e) => {
                    log::error!("invalid /api/watch URL: {e:#}");
                    return;
                }
            };
            loop {
                match watch_loop(&http, &url, &tx).await {
                    Ok(()) => {
                        log::info!("watch stream closed by server");
                        break;
                    }
                    Err(e) => {
                        log::warn!("watch stream error: {e:#}; reconnecting in 2s");
                    }
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        });
        rx
    }

    /// Hold the `/api/heartbeat` connection open for the life of the
    /// process. Without this, the server shuts down 100ms after the last
    /// client disconnects (unless launched with `--keep-alive`).
    pub fn start_heartbeat(&self) {
        let http = self.http.clone();
        let url = self.base_url.join("/api/heartbeat");
        self.runtime.spawn(async move {
            let url = match url {
                Ok(u) => u,
                Err(e) => {
                    log::error!("invalid /api/heartbeat URL: {e:#}");
                    return;
                }
            };
            loop {
                match heartbeat_loop(&http, &url).await {
                    Ok(()) => {
                        log::info!("heartbeat closed by server");
                        break;
                    }
                    Err(e) => {
                        log::warn!("heartbeat error: {e:#}; reconnecting in 2s");
                    }
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        });
    }

    fn post_json<Q, B>(
        &self,
        path: &str,
        query: &Q,
        body: &B,
    ) -> oneshot::Receiver<Result<()>>
    where
        Q: Serialize + Sized,
        B: Serialize + Sized,
    {
        let (tx, rx) = oneshot::channel();
        let url = match self.base_url.join(path) {
            Ok(u) => u,
            Err(e) => {
                let _ = tx.send(Err(anyhow!(e).context("invalid url")));
                return rx;
            }
        };
        let qs = match serde_urlencoded::to_string(query) {
            Ok(s) => s,
            Err(e) => {
                let _ = tx.send(Err(anyhow!(e).context("encoding query")));
                return rx;
            }
        };
        let body_bytes = match serde_json::to_vec(body) {
            Ok(b) => b,
            Err(e) => {
                let _ = tx.send(Err(anyhow!(e).context("encoding body")));
                return rx;
            }
        };
        let target = if qs.is_empty() {
            url
        } else {
            let mut u = url;
            u.set_query(Some(&qs));
            u
        };
        let http = self.http.clone();
        self.runtime.spawn(async move {
            let result = async {
                let resp = http
                    .post(target.clone())
                    .header("Content-Type", "application/json")
                    .body(body_bytes)
                    .send()
                    .await
                    .with_context(|| format!("POST {target}"))?;
                if !resp.status().is_success() {
                    return Err(anyhow!(
                        "POST {target} failed: HTTP {}",
                        resp.status().as_u16()
                    ));
                }
                Ok(())
            }
            .await;
            let _ = tx.send(result);
        });
        rx
    }

    fn get_json<Q, T>(&self, path: &str, query: &Q) -> oneshot::Receiver<Result<T>>
    where
        Q: Serialize + Sized,
        T: for<'de> serde::Deserialize<'de> + Send + 'static,
    {
        let (tx, rx) = oneshot::channel();
        let url = match self.base_url.join(path) {
            Ok(u) => u,
            Err(e) => {
                let _ = tx.send(Err(anyhow!(e).context("invalid url")));
                return rx;
            }
        };
        let qs = match serde_urlencoded::to_string(query) {
            Ok(s) => s,
            Err(e) => {
                let _ = tx.send(Err(anyhow!(e).context("encoding query")));
                return rx;
            }
        };
        let http = self.http.clone();
        let target = if qs.is_empty() {
            url
        } else {
            let mut u = url;
            u.set_query(Some(&qs));
            u
        };
        self.runtime.spawn(async move {
            let result = async {
                let resp = http
                    .get(target.clone())
                    .send()
                    .await
                    .with_context(|| format!("GET {target}"))?;
                if !resp.status().is_success() {
                    return Err(anyhow!(
                        "GET {target} failed: HTTP {}",
                        resp.status().as_u16()
                    ));
                }
                let parsed: T = resp
                    .json()
                    .await
                    .with_context(|| format!("decoding response from {target}"))?;
                Ok(parsed)
            }
            .await;
            let _ = tx.send(result);
        });
        rx
    }
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct DiffQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "baseMode")]
    pub base_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "ignoreWhitespace")]
    pub ignore_whitespace: Option<bool>,
}

impl DiffQuery {
    pub fn from_selection(base: Option<&str>, target: Option<&str>) -> Self {
        Self {
            base: base.map(str::to_string),
            target: target.map(str::to_string),
            base_mode: None,
            ignore_whitespace: None,
        }
    }
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct CommentSelectionQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "baseMode")]
    pub base_mode: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize)]
struct EmptyQuery;

async fn watch_loop(
    http: &Client,
    url: &Url,
    tx: &mpsc::UnboundedSender<WatchEvent>,
) -> Result<()> {
    let mut resp = http
        .get(url.clone())
        .header("Accept", "text/event-stream")
        .send()
        .await?
        .error_for_status()?;
    let mut buf: Vec<u8> = Vec::with_capacity(1024);
    while let Some(chunk) = resp.chunk().await? {
        buf.extend_from_slice(&chunk);
        while let Some(idx) = find_event_boundary(&buf) {
            let raw = buf.drain(..idx + 2).collect::<Vec<u8>>();
            // The drained bytes still include the trailing "\n\n"; trim it.
            let raw = &raw[..raw.len() - 2];
            if let Ok(text) = std::str::from_utf8(raw) {
                if let Some(event) = parse_sse_event(text) {
                    if tx.send(event).is_err() {
                        // Receiver dropped — stop.
                        return Ok(());
                    }
                }
            }
        }
    }
    Ok(())
}

async fn heartbeat_loop(http: &Client, url: &Url) -> Result<()> {
    let mut resp = http
        .get(url.clone())
        .header("Accept", "text/event-stream")
        .send()
        .await?
        .error_for_status()?;
    while let Some(_chunk) = resp.chunk().await? {
        // Drain & discard; the only purpose is to keep the TCP connection
        // alive so the server doesn't exit.
    }
    Ok(())
}

fn find_event_boundary(buf: &[u8]) -> Option<usize> {
    buf.windows(2).position(|w| w == b"\n\n")
}

fn parse_sse_event(text: &str) -> Option<WatchEvent> {
    // Concatenate all `data:` lines per the SSE spec.
    let mut data = String::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(rest.trim_start());
        }
    }
    if data.is_empty() {
        return None;
    }

    #[derive(serde::Deserialize)]
    struct EventEnvelope {
        #[serde(rename = "type")]
        kind: Option<String>,
        version: Option<u64>,
    }

    match serde_json::from_str::<EventEnvelope>(&data) {
        Ok(env) => match env.kind.as_deref() {
            Some("filesChanged") => Some(WatchEvent::FilesChanged),
            Some("commentsChanged") => Some(WatchEvent::CommentsChanged {
                version: env.version.unwrap_or(0),
            }),
            Some(other) => Some(WatchEvent::Other(other.to_string())),
            None => Some(WatchEvent::Other(data)),
        },
        Err(_) => Some(WatchEvent::Other(data)),
    }
}
