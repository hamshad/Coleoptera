mod crawler;
pub mod server;
pub mod state;

use std::sync::Arc;

use tauri::Manager;
use tauri::WebviewUrl;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Fixed port for the embedded HTTP backend (Axum). The webview window
    // loads from this same origin so the frontend can call the API with
    // plain same-origin fetch (no CORS needed).
    const PORT: u16 = 1420;

    tauri::Builder::default()
        .setup(move |app| {
            // Remove the auto-created window; we recreate it pointing at the
            // local HTTP server so frontend + backend share one origin.
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.destroy();
            }

            let state = Arc::new(state::AppState::new());
            let router = server::router(state);
            let addr: std::net::SocketAddr = ([127, 0, 0, 1], PORT).into();
            tauri::async_runtime::spawn(async move {
                let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
                axum::serve(listener, router).await.unwrap();
            });

            // Give the server a moment, then open the webview at the server URL.
            let url = format!("http://127.0.0.1:{}/", PORT);
            let _main = tauri::WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url.parse().unwrap()))
                .title("Coleoptera")
                .inner_size(1100.0, 800.0)
                .min_inner_size(700.0, 500.0)
                .resizable(true)
                .center()
                .build()?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Coleoptera");
}
