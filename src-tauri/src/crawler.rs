use std::ffi::{CStr, CString};

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Sender;
use tokio::sync::watch;

/// Options the frontend can pass when starting a crawl, mirroring the
/// original Flask backend's `options` dict (slimmed to what a fetch-based
/// engine supports).
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct CrawlOptions {
    #[serde(default)]
    pub stealth_mode: bool,
    #[serde(default)]
    pub remove_overlays: bool,
    #[serde(default)]
    pub remove_consent: bool,
    #[serde(default)]
    pub user_agent: Option<String>,
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    #[serde(default)]
    pub max_retries: u32,
}

#[derive(Debug, Deserialize)]
pub struct CrawlRequest {
    pub url: String,
    #[serde(default)]
    pub options: CrawlOptions,
}

fn emit(tx: &Sender<String>, value: serde_json::Value) {
    // Send only the JSON; axum's `Event::data()` adds the `data: ` SSE prefix
    // and the trailing blank line, so we must NOT prefix it here.
    let line = value.to_string();
    let _ = tx.blocking_send(line);
}

fn normalize_url(raw: &str) -> String {
    let u = raw.trim();
    if u.starts_with("http://") || u.starts_with("https://") {
        u.to_string()
    } else {
        format!("https://{}", u)
    }
}

/// Run the crawl. Progress is pushed as SSE `data:` lines through `tx`.
/// `cancel` is watched for a `true` value to abort mid-crawl.
pub fn crawl(
    req: CrawlRequest,
    session_id: String,
    tx: Sender<String>,
    cancel: watch::Receiver<bool>,
) {
    let url = normalize_url(&req.url);

    emit(
        &tx,
        serde_json::json!({
            "event": "start",
            "session_id": session_id,
            "url": url,
            "options": req.options
        }),
    );

    emit(
        &tx,
        serde_json::json!({
            "event": "log",
            "msg": format!("Fetching {}", url),
            "level": "info"
        }),
    );

    emit(
        &tx,
        serde_json::json!({ "event": "progress", "status": "page_loading", "url": url }),
    );

    let timeout = std::time::Duration::from_secs(req.options.timeout_secs.unwrap_or(30));
    let ua = req
        .options
        .user_agent
        .clone()
        .unwrap_or_else(|| "Coleoptera/1.0 (+https://github.com/coleoptera)".to_string());

    let client = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .user_agent(ua)
        .gzip(true)
        .brotli(true)
        .deflate(true)
        .build();

    let client = match client {
        Ok(c) => c,
        Err(e) => {
            emit(
                &tx,
                serde_json::json!({ "event": "done", "success": false, "error": format!("client init failed: {}", e) }),
            );
            return;
        }
    };

    let mut attempts = 0u32;
    let max_attempts = req.options.max_retries.max(1);
    let mut last_error: Option<String>;

    loop {
        if *cancel.borrow() {
            emit(
                &tx,
                serde_json::json!({ "event": "done", "success": false, "error": "cancelled" }),
            );
            return;
        }

        match client.get(&url).send() {
            Ok(resp) => {
                let status = resp.status();
                emit(
                    &tx,
                    serde_json::json!({
                        "event": "log",
                        "msg": format!("HTTP {}", status),
                        "level": "info"
                    }),
                );

                if !status.is_success() {
                    last_error = Some(format!("HTTP status {}", status));
                    attempts += 1;
                    if attempts >= max_attempts {
                        emit(
                            &tx,
                            serde_json::json!({ "event": "done", "success": false, "error": last_error }),
                        );
                        return;
                    }
                    continue;
                }

                let html = match resp.text() {
                    Ok(t) => t,
                    Err(e) => {
                        emit(
                            &tx,
                            serde_json::json!({ "event": "done", "success": false, "error": format!("read body failed: {}", e) }),
                        );
                        return;
                    }
                };

                emit(
                    &tx,
                    serde_json::json!({ "event": "progress", "status": "page_loaded" }),
                );
                emit(
                    &tx,
                    serde_json::json!({ "event": "log", "msg": "Page loaded, extracting content...", "level": "info" }),
                );
                emit(
                    &tx,
                    serde_json::json!({ "event": "progress", "status": "extracting" }),
                );

                let markdown = convert_to_markdown(&html);

                emit(
                    &tx,
                    serde_json::json!({ "event": "progress", "status": "html_retrieved" }),
                );
                emit(
                    &tx,
                    serde_json::json!({
                        "event": "log",
                        "msg": format!("Content extracted: {} chars", markdown.chars().count()),
                        "level": "info"
                    }),
                );

                emit(
                    &tx,
                    serde_json::json!({ "event": "done", "success": true, "markdown": markdown }),
                );
                return;
            }
            Err(e) => {
                last_error = Some(format!("{}", e));
                attempts += 1;
                if attempts >= max_attempts {
                    emit(
                        &tx,
                        serde_json::json!({
                            "event": "log",
                            "msg": "Anti-bot / network block detected. Try a different User-Agent or a proxy.",
                            "level": "warning"
                        }),
                    );
                    emit(
                        &tx,
                        serde_json::json!({ "event": "done", "success": false, "error": last_error }),
                    );
                    return;
                }
            }
        }
    }
}

/// Strip unwanted elements, then convert the cleaned HTML to Markdown via
/// html2md's C ABI.
fn convert_to_markdown(html: &str) -> String {
    let parsed = scraper::Html::parse_document(html);

    // Find a <base href> (used to annotate relative links).
    let base = parsed
        .select(&scraper::Selector::parse("base").unwrap())
        .next()
        .and_then(|b| b.attr("href").map(|s| s.to_string()));

    // Prefer the main readable region when present; otherwise the <body>.
    let scope_html = parsed
        .select(&scraper::Selector::parse("main, article").unwrap())
        .next()
        .or_else(|| parsed.select(&scraper::Selector::parse("body").unwrap()).next())
        .map(|e| e.html())
        .unwrap_or_else(|| html.to_string());

    // Remove noisy / non-content subtrees before conversion so raw HTML and
    // script/style/svg markup don't leak into the Markdown output.
    let mut doc = scraper::Html::parse_document(&scope_html);
    for sel in [
        "script", "style", "noscript", "svg", "template", "head", "iframe",
        "nav", "header", "footer", "aside", "form", "button", "noscript",
        "meta", "link",
    ] {
        if let Ok(selector) = scraper::Selector::parse(sel) {
            let mut removed_any = true;
            while removed_any {
                removed_any = false;
                if let Some(node) = doc.select(&selector).next() {
                    let id = node.id();
                    if doc.tree.get_mut(id).is_some() {
                        doc.tree.remove(id);
                        removed_any = true;
                    }
                }
            }
        }
    }

    let cleaned = doc.root_element().html();

    // Drop HTML comments (html2md may otherwise pass them through literally).
    let cleaned = regex::Regex::new(r"<!--.*?-->")
        .map(|re| re.replace_all(&cleaned, "").to_string())
        .unwrap_or(cleaned);

    let c_html = match CString::new(cleaned) {
        Ok(c) => c,
        Err(_) => return String::new(),
    };
    // html2md 0.2 exposes a C-ABI `parse(*const c_char) -> *const c_char`.
    let c_ptr = c_html.as_ptr();
    let ptr = unsafe { html2md::parse(c_ptr) };
    if ptr.is_null() {
        return String::new();
    }
    let markdown = unsafe {
        let c_str = CStr::from_ptr(ptr);
        c_str.to_string_lossy().into_owned()
    };
    // html2md allocates with libc malloc; free to avoid a leak.
    unsafe {
        libc::free(ptr as *mut libc::c_void);
    }

    // Collapse 3+ blank lines that html2md sometimes leaves behind.
    let markdown = regex::Regex::new(r"\n{3,}")
        .map(|re| re.replace_all(&markdown, "\n\n").to_string())
        .unwrap_or(markdown);

    if let Some(b) = base {
        format!("> Source base: {}\n\n{}", b, markdown.trim())
    } else {
        markdown.trim().to_string()
    }
}
