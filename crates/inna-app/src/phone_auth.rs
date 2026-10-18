//! In-app phone login: LAN reverse-proxy with no extra windows.
//!
//! Phone opens `http://DEVICE:PORT/pair`, then browses INNA through this
//! process. Relative `/api/...` calls hit our fallback proxy so cookies are
//! captured. When GetLoggedInUser succeeds, we save the session.

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use inna_core::endpoints::INNA_BASE_URL;
use inna_core::session::{SessionCookie, StoredSession};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

const LISTEN_PORT: u16 = 8765;

#[derive(Clone)]
struct ProxyState {
    cookies: Arc<Mutex<HashMap<String, SessionCookie>>>,
    http: reqwest::Client,
    done: Arc<AtomicBool>,
    session: Arc<Mutex<Option<StoredSession>>>,
}

pub struct PhoneAuthHandle {
    pub url: String,
    done: Arc<AtomicBool>,
    session: Arc<Mutex<Option<StoredSession>>>,
}

impl PhoneAuthHandle {
    pub fn is_done(&self) -> bool {
        self.done.load(Ordering::SeqCst)
    }

    pub fn take_session(&self) -> Option<StoredSession> {
        self.session.lock().unwrap().take()
    }
}

/// Start the LAN proxy. Returns the pair URL immediately; poll `handle` for completion.
pub async fn start_phone_auth() -> Result<PhoneAuthHandle, String> {
    let lan_ip = local_lan_ip().unwrap_or_else(|| "127.0.0.1".into());
    let pair_url = format!("http://{lan_ip}:{LISTEN_PORT}/pair");

    let state = ProxyState {
        cookies: Arc::new(Mutex::new(HashMap::new())),
        http: reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(45))
            .build()
            .map_err(|e| e.to_string())?,
        done: Arc::new(AtomicBool::new(false)),
        session: Arc::new(Mutex::new(None)),
    };

    {
        let state = state.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                if state.done.load(Ordering::SeqCst) {
                    break;
                }
                if let Some(session) = try_validate(&state).await {
                    tracing::info!("phone auth validated ({} cookies)", session.cookies.len());
                    *state.session.lock().unwrap() = Some(session);
                    state.done.store(true, Ordering::SeqCst);
                    break;
                }
            }
        });
    }

    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], LISTEN_PORT)))
        .await
        .map_err(|e| {
            format!("cannot bind port {LISTEN_PORT}: {e} (is another login already running?)")
        })?;

    let app = Router::new()
        .route("/pair", get(landing))
        .route("/status", get(status))
        // Everything else — including /api/* — proxies to INNA
        .fallback(proxy)
        .with_state(state.clone());

    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("phone auth server error: {e}");
        }
    });

    tracing::info!("phone auth listening at {pair_url}");

    Ok(PhoneAuthHandle {
        url: pair_url,
        done: state.done.clone(),
        session: state.session.clone(),
    })
}

async fn landing(State(state): State<ProxyState>) -> Html<String> {
    if state.done.load(Ordering::SeqCst) {
        return Html(
            "<html><body style='font-family:sans-serif;background:#f2f5fa;color:#1b2b42;padding:2rem'>\
             <h1 style='color:#197a57'>Connected</h1><p>You can close this tab.</p></body></html>"
                .into(),
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
<p>Choose the button below and sign in. Keep this page open until Anni shows your planner.</p>
<a class="btn" href="/">Open INNA</a>
<script>
setInterval(async()=>{
  try{const r=await fetch('/status');const j=await r.json();
  if(j.authenticated){document.body.innerHTML='<h1 style=color:#197a57>Connected</h1><p>You can close this tab.</p>';}}catch(e){}
},1500);
</script>
</body></html>"#
            .into(),
    )
}

async fn status(State(state): State<ProxyState>) -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "authenticated": state.done.load(Ordering::SeqCst),
        "cookies": state.cookies.lock().unwrap().len(),
    }))
}

async fn proxy(State(state): State<ProxyState>, req: Request) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let headers = req.headers().clone();
    let body_bytes = match axum::body::to_bytes(req.into_body(), 20 * 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => return (StatusCode::BAD_REQUEST, format!("body: {e}")).into_response(),
    };

    // Skip our own control paths if they somehow hit fallback
    let path = uri.path();
    if path == "/pair" || path == "/status" {
        return StatusCode::NOT_FOUND.into_response();
    }

    let target_path = match uri.query() {
        Some(q) => format!("{path}?{q}"),
        None => path.to_string(),
    };
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
    // Forward form posts as the browser would
    if let Some(v) = headers.get(header::USER_AGENT) {
        builder = builder.header(header::USER_AGENT, v);
    }

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

    // XSRF for API calls if we have the token
    if let Some(token) = state.cookies.lock().unwrap().get("XSRF-TOKEN") {
        let decoded = percent_encoding::percent_decode_str(&token.value)
            .decode_utf8_lossy()
            .into_owned();
        builder = builder
            .header("X-XSRF-TOKEN", decoded)
            .header("X-Requested-By", "XMLHttpRequest");
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
                tracing::debug!("captured cookie {}", cookie.name);
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
                | "strict-transport-security"
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

    // Re-emit cookies for the phone (HTTP, host-only on device IP)
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

    let body = if content_type.contains("text/html") || content_type.contains("javascript") {
        let text = String::from_utf8_lossy(&bytes);
        let rewritten = text
            .replace("https://nam.inna.is", "")
            .replace("http://nam.inna.is", "");
        Body::from(rewritten)
    } else {
        Body::from(bytes)
    };

    let mut resp = Response::new(body);
    *resp.status_mut() = status;
    *resp.headers_mut() = response_headers;
    resp
}

fn rewrite_location(loc: &str) -> String {
    if let Some(rest) = loc.strip_prefix(INNA_BASE_URL) {
        return rest.to_string();
    }
    if let Some(rest) = loc.strip_prefix("http://nam.inna.is") {
        return rest.to_string();
    }
    loc.to_string()
}

fn parse_set_cookie(raw: &str) -> Option<SessionCookie> {
    let mut parts = raw.split(';');
    let nv = parts.next()?.trim();
    let (name, value) = nv.split_once('=')?;
    if name.is_empty() {
        return None;
    }
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
        if jar.len() < 2 {
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
