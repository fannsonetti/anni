//! Phone QR authentication via a LAN reverse-proxy.
//!
//! The locked device cannot open an interactive browser, but can still reach
//! INNA over HTTPS. The phone browses through `http://DEVICE:PORT/browse/…`
//! which proxies to nam.inna.is. Captured cookies become a StoredSession.

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{any, get};
use axum::Router;
use inna_core::endpoints::INNA_BASE_URL;
use inna_core::session::{SessionCookie, StoredSession};
use qrcode::render::svg;
use qrcode::QrCode;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

const LISTEN_PORT: u16 = 8765;

#[derive(Clone)]
struct ProxyState {
    cookies: Arc<Mutex<HashMap<String, SessionCookie>>>,
    http: reqwest::Client,
    done: Arc<AtomicBool>,
    session: Arc<Mutex<Option<StoredSession>>>,
}

pub fn run_phone_login() -> ! {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");

    let lan_ip = local_lan_ip().unwrap_or_else(|| "127.0.0.1".into());
    let pair_url = format!("http://{lan_ip}:{LISTEN_PORT}/");
    tracing::info!("phone auth listening at {pair_url}");

    let state = ProxyState {
        cookies: Arc::new(Mutex::new(HashMap::new())),
        http: reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("http client"),
        done: Arc::new(AtomicBool::new(false)),
        session: Arc::new(Mutex::new(None)),
    };

    {
        let state = state.clone();
        rt.spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                if state.done.load(Ordering::SeqCst) {
                    break;
                }
                if let Some(session) = try_validate(&state).await {
                    tracing::info!("phone auth validated");
                    *state.session.lock().unwrap() = Some(session);
                    state.done.store(true, Ordering::SeqCst);
                    break;
                }
            }
        });
    }

    {
        let state = state.clone();
        rt.spawn(async move {
            // axum 0.7 catch-all syntax is `/*path` (not `/{*path}`)
            let app = Router::new()
                .route("/", get(landing))
                .route("/status", get(status))
                .route("/browse", any(proxy))
                .route("/browse/", any(proxy))
                .route("/browse/*path", any(proxy))
                .with_state(state);

            let addr = SocketAddr::from(([0, 0, 0, 0], LISTEN_PORT));
            let listener = tokio::net::TcpListener::bind(addr)
                .await
                .expect("bind phone auth port — is 8765 already in use?");
            axum::serve(listener, app).await.expect("serve");
        });
    }

    let qr_html = build_qr_page(&pair_url, &lan_ip);
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy_evt = event_loop.create_proxy();

    {
        let done = state.done.clone();
        let proxy_evt = proxy_evt.clone();
        std::thread::spawn(move || {
            while !done.load(Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_millis(400));
            }
            let _ = proxy_evt.send_event(UserEvent::AuthCompleted);
        });
    }

    let window = WindowBuilder::new()
        .with_title("anni · Sign in with your phone")
        .with_inner_size(tao::dpi::LogicalSize::new(480.0, 640.0))
        .build(&event_loop)
        .expect("window");

    let _webview = WebViewBuilder::new()
        .with_html(&qr_html)
        .build(&window)
        .expect("webview");

    let session_slot = state.session.clone();
    event_loop.run(move |event, _target, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(UserEvent::AuthCompleted) => {
                if let Some(session) = session_slot.lock().unwrap().clone() {
                    let json = serde_json::to_string(&session).expect("serialize");
                    println!("{json}");
                    tracing::info!("authentication completed successfully");
                    std::process::exit(0);
                }
                tracing::error!("auth completed but no session");
                std::process::exit(1);
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                tracing::error!("phone login window closed");
                std::process::exit(1);
            }
            _ => {}
        }
    });
}

#[derive(Debug)]
enum UserEvent {
    AuthCompleted,
}

async fn landing(State(state): State<ProxyState>) -> Html<String> {
    if state.done.load(Ordering::SeqCst) {
        return Html(
            "<html><body style='font-family:sans-serif;padding:2rem'><h1>Connected</h1><p>You can close this tab.</p></body></html>"
                .to_string(),
        );
    }
    Html(
        r#"<!DOCTYPE html>
<html><head><meta name="viewport" content="width=device-width,initial-scale=1">
<title>anni · Sign in to INNA</title>
<style>
body{font-family:system-ui,sans-serif;background:#f2f5fa;color:#1b2b42;margin:0;padding:2rem;line-height:1.5}
a.btn{display:inline-block;background:#007080;color:#fff;text-decoration:none;padding:1rem 1.5rem;border-radius:8px;font-size:1.2rem;margin-top:1rem}
p{color:#5b6b80}
</style></head><body>
<h1>Connect INNA on your phone</h1>
<p>Choose the button below and sign in to INNA. Your device will connect automatically when sign-in is complete.</p>
<a class="btn" href="/browse/">Open INNA</a>
</body></html>"#
            .to_string(),
    )
}

async fn status(State(state): State<ProxyState>) -> impl IntoResponse {
    let done = state.done.load(Ordering::SeqCst);
    axum::Json(serde_json::json!({ "authenticated": done }))
}

async fn proxy(State(state): State<ProxyState>, req: Request) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let headers = req.headers().clone();
    let body_bytes = match axum::body::to_bytes(req.into_body(), 20 * 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, format!("body: {e}")).into_response();
        }
    };

    let target_path = strip_browse_prefix(uri.path(), uri.query());
    let target_url = format!("{INNA_BASE_URL}{target_path}");

    let reqwest_method =
        reqwest::Method::from_bytes(method.as_str().as_bytes()).unwrap_or(reqwest::Method::GET);

    let mut builder = state.http.request(reqwest_method, &target_url);

    if let Some(v) = headers.get(header::ACCEPT) {
        builder = builder.header(header::ACCEPT, v);
    }
    if let Some(v) = headers.get(header::CONTENT_TYPE) {
        builder = builder.header(header::CONTENT_TYPE, v);
    }
    if let Some(v) = headers.get(header::ACCEPT_LANGUAGE) {
        builder = builder.header(header::ACCEPT_LANGUAGE, v);
    }
    builder = builder.header("X-Requested-By", "XMLHttpRequest");

    let cookie_header = {
        let jar = state.cookies.lock().unwrap();
        jar.values()
            .map(|c| format!("{}={}", c.name, c.value))
            .collect::<Vec<_>>()
            .join("; ")
    };
    if !cookie_header.is_empty() {
        builder = builder.header(header::COOKIE, cookie_header);
    }

    if !body_bytes.is_empty() && method != Method::GET && method != Method::HEAD {
        builder = builder.body(body_bytes.to_vec());
    }

    let upstream = match builder.send().await {
        Ok(r) => r,
        Err(e) => {
            return (StatusCode::BAD_GATEWAY, format!("upstream: {e}")).into_response();
        }
    };

    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let upstream_headers = upstream.headers().clone();

    for val in upstream_headers.get_all(header::SET_COOKIE) {
        if let Ok(s) = val.to_str() {
            if let Some(cookie) = parse_set_cookie(s) {
                state
                    .cookies
                    .lock()
                    .unwrap()
                    .insert(cookie.name.clone(), cookie);
            }
        }
    }

    let bytes = match upstream.bytes().await {
        Ok(b) => b,
        Err(e) => {
            return (StatusCode::BAD_GATEWAY, format!("read: {e}")).into_response();
        }
    };

    let mut response_headers = HeaderMap::new();
    for (name, value) in upstream_headers.iter() {
        let n = name.as_str();
        if matches!(
            n,
            "set-cookie"
                | "content-encoding"
                | "content-length"
                | "transfer-encoding"
                | "content-security-policy"
                | "x-frame-options"
        ) {
            continue;
        }
        if n == "location" {
            if let Ok(loc) = value.to_str() {
                if let Ok(hv) = HeaderValue::from_str(&rewrite_location(loc)) {
                    response_headers.insert(header::LOCATION, hv);
                }
                continue;
            }
        }
        if let Ok(hn) = HeaderName::from_bytes(name.as_str().as_bytes()) {
            response_headers.insert(hn, value.clone());
        }
    }

    for cookie in state.cookies.lock().unwrap().values() {
        let set = format!(
            "{}={}; Path={}; SameSite=Lax",
            cookie.name, cookie.value, cookie.path
        );
        if let Ok(hv) = HeaderValue::from_str(&set) {
            response_headers.append(header::SET_COOKIE, hv);
        }
    }

    let content_type = response_headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let body = if content_type.contains("text/html") {
        let text = String::from_utf8_lossy(&bytes);
        let rewritten = text
            .replace("https://nam.inna.is/", "/browse/")
            .replace("https://nam.inna.is", "/browse/")
            .replace("http://nam.inna.is/", "/browse/")
            .replace("http://nam.inna.is", "/browse/");
        Body::from(rewritten)
    } else {
        Body::from(bytes)
    };

    let mut resp = Response::new(body);
    *resp.status_mut() = status;
    *resp.headers_mut() = response_headers;
    resp
}

fn strip_browse_prefix(path: &str, query: Option<&str>) -> String {
    let rest = path
        .strip_prefix("/browse/")
        .or_else(|| path.strip_prefix("/browse"))
        .unwrap_or("");
    let path = if rest.is_empty() {
        "/".to_string()
    } else if rest.starts_with('/') {
        rest.to_string()
    } else {
        format!("/{rest}")
    };
    match query {
        Some(q) => format!("{path}?{q}"),
        None => path,
    }
}

fn rewrite_location(loc: &str) -> String {
    if let Some(rest) = loc.strip_prefix(INNA_BASE_URL) {
        return format!("/browse{rest}");
    }
    if let Some(rest) = loc.strip_prefix("http://nam.inna.is") {
        return format!("/browse{rest}");
    }
    if loc.starts_with('/') {
        return format!("/browse{loc}");
    }
    // External IdP — phone leaves the proxy; may need local login instead
    loc.to_string()
}

fn parse_set_cookie(raw: &str) -> Option<SessionCookie> {
    let mut parts = raw.split(';');
    let nv = parts.next()?.trim();
    let (name, value) = nv.split_once('=')?;
    let mut path = "/".to_string();
    let mut domain = "nam.inna.is".to_string();
    let mut http_only = false;
    let mut secure = false;
    for p in parts {
        let p = p.trim();
        if let Some(v) = p.strip_prefix("Path=").or_else(|| p.strip_prefix("path=")) {
            path = v.to_string();
        } else if let Some(v) = p
            .strip_prefix("Domain=")
            .or_else(|| p.strip_prefix("domain="))
        {
            domain = v.trim_start_matches('.').to_string();
        } else if p.eq_ignore_ascii_case("HttpOnly") {
            http_only = true;
        } else if p.eq_ignore_ascii_case("Secure") {
            secure = true;
        }
    }
    Some(SessionCookie {
        name: name.to_string(),
        value: value.to_string(),
        domain,
        path,
        http_only,
        secure,
    })
}

async fn try_validate(state: &ProxyState) -> Option<StoredSession> {
    let cookies: Vec<SessionCookie> = {
        let jar = state.cookies.lock().unwrap();
        if jar.is_empty() {
            return None;
        }
        jar.values().cloned().collect()
    };

    let session = StoredSession {
        cookies,
        saved_at: chrono::Utc::now().to_rfc3339(),
    };

    let client = inna_core::client::InnaClient::new().ok()?;
    client.load_session(&session);
    match client.current_user().await {
        Ok(user) if user.student_name.is_some() || user.student_id.is_some() => Some(session),
        _ => None,
    }
}

fn local_lan_ip() -> Option<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let ip = socket.local_addr().ok()?.ip();
    if ip.is_loopback() {
        return None;
    }
    Some(ip.to_string())
}

fn build_qr_page(pair_url: &str, lan_ip: &str) -> String {
    let qr_svg = match QrCode::new(pair_url.as_bytes()) {
        Ok(code) => code
            .render::<svg::Color>()
            .min_dimensions(200, 200)
            .dark_color(svg::Color("#000000"))
            .light_color(svg::Color("#ffffff"))
            .build(),
        Err(_) => String::new(),
    };

    format!(
        r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>anni · Sign in with your phone</title>
<style>
body{{margin:0;background:#f2f5fa;color:#1b2b42;font-family:system-ui,sans-serif;display:flex;flex-direction:column;align-items:center;padding:24px;text-align:center}}
h1{{font-size:1.4rem;margin:0 0 8px}}
p{{color:#5b6b80;max-width:400px;line-height:1.4}}
.url{{font-size:1.35rem;color:#007080;word-break:break-all;background:#e2f0f4;padding:12px 16px;border-radius:8px;margin:16px 0;user-select:all}}
.qr{{background:#fff;padding:10px;border-radius:8px;margin:12px 0;opacity:0.95}}
small{{color:#666}}
</style></head><body>
<h1>Sign in with your phone</h1>
<p>Open this address on your phone on the same Wi-Fi network and sign in to INNA. This window closes once you're connected.</p>
<div class="url">{pair_url}</div>
<p><small>QR code</small></p>
<div class="qr">{qr_svg}</div>
<p><small>{lan_ip}:{LISTEN_PORT}</small></p>
<script>
setInterval(async()=>{{
  try{{const r=await fetch('/status');const j=await r.json();
  if(j.authenticated) document.body.insertAdjacentHTML('afterbegin','<h1 style=color:#197a57>Connected</h1>');}}catch(e){{}}
}},1500);
</script>
</body></html>"#
    )
}
