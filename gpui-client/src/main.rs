use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use gpui::{px, size, App, Bounds, WindowBounds, WindowOptions};
use gpui_platform::application;
use url::Url;

mod api;
mod app;
mod highlighting;
mod ui;

use crate::api::ApiClient;
use crate::app::DifitApp;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "difit-gpui",
    about = "Native GPUI client for the difit diff/review server"
)]
struct Cli {
    /// Base URL of the running difit server.
    #[arg(long, env = "DIFIT_SERVER", default_value = "http://127.0.0.1:4966")]
    server: String,
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();
    let base_url = Url::parse(&cli.server)?;

    // We keep a dedicated tokio runtime for reqwest. GPUI runs its UI loop
    // on its own executor; HTTP/I/O calls are spawned onto tokio and the
    // results are funneled back through oneshot channels.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .thread_name("difit-gpui-net")
        .build()?;
    let runtime_handle = runtime.handle().clone();

    // Keep the runtime alive for the lifetime of the process.
    std::mem::forget(runtime);

    let api = Arc::new(ApiClient::new(base_url, runtime_handle));

    application().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1280.0), px(800.0)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some("difit".into()),
                ..Default::default()
            }),
            ..Default::default()
        };

        cx.open_window(options, |window, cx| {
            DifitApp::new(api.clone(), window, cx)
        })
        .expect("failed to open window");

        cx.activate(true);
    });

    Ok(())
}
