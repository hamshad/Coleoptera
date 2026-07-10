// Standalone end-to-end check: binds the app router, hits /health, /, and
// /crawl/stream for a real URL, and prints the SSE events it receives.
// Run with: cargo run --example e2e
use std::sync::Arc;

use coleoptera_lib::server::router;
use coleoptera_lib::state::AppState;
use futures::StreamExt;

#[tokio::main]
async fn main() {
    let state = Arc::new(AppState::new());
    let app = router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8753").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let base = format!("http://{}", addr);
    let client = reqwest::Client::new();

    // /health
    let h: serde_json::Value = client.get(format!("{}/health", base)).send().await.unwrap().json().await.unwrap();
    println!("[health] {}", h);

    // /
    let idx = client.get(&base).send().await.unwrap().text().await.unwrap();
    println!("[index] {} chars, starts with <!DOCTYPE: {}", idx.len(), idx.trim_start().starts_with("<!DOCTYPE"));

    // /crawl/stream
    let target = std::env::args().nth(1).unwrap_or_else(|| "https://example.com".to_string());
    println!("[crawl] requesting {}", target);
    let resp = client
        .post(format!("{}/crawl/stream", base))
        .json(&serde_json::json!({ "url": target, "options": {} }))
        .send()
        .await
        .unwrap();
    println!("[crawl] status={} headers={:?}", resp.status(), resp.headers());

    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    let mut done = false;
    let mut total: usize = 0;
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.unwrap();
        total += bytes.len();
        println!("[raw] chunk {} bytes (total {})", bytes.len(), total);
        if total <= 242 {
            println!("[hex] {:?}", String::from_utf8_lossy(&bytes));
        }
        buf.push_str(&String::from_utf8_lossy(&bytes));
        while let Some(idx) = buf.find("\n\n") {
            let frame = buf[..idx].to_string();
            buf = buf[idx + 2..].to_string();
            if let Some(data) = frame.lines().find(|l| l.starts_with("data:")) {
                let payload = &data[5..];
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(payload) {
                    match v["event"].as_str() {
                        Some("start") => println!("[sse] start url={}", v["url"]),
                        Some("progress") => println!("[sse] progress {}", v["status"]),
                        Some("log") => println!("[sse] log[{}] {}", v["level"], v["msg"]),
                        Some("done") => {
                            let ok = v["success"].as_bool().unwrap_or(false);
                            if ok {
                                let md = v["markdown"].as_str().unwrap_or("");
                                println!("[sse] done OK, markdown {} chars", md.chars().count());
                                println!("---- MARKDOWN PREVIEW ----\n{}\n---- END ----", &md[..md.char_indices().nth(400).map(|(i,_)| i).unwrap_or(md.len())]);
                            } else {
                                println!("[sse] done FAIL: {}", v["error"]);
                            }
                            done = true;
                        }
                        _ => println!("[sse] {}", payload),
                    }
                }
            }
            if done { break; }
        }
        if done { break; }
    }
    println!("[e2e] finished, total bytes {}", total);
}
