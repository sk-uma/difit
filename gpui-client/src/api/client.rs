use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::Serialize;
use tokio::sync::oneshot;
use url::Url;

use super::types::{CommentsJsonResponse, DiffResponse, RevisionsResponse};

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

    pub fn base_url(&self) -> &Url {
        &self.base_url
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
