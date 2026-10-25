//! Explicit INNA-only session handoff from the user's browser extension over loopback.
use axum::{
    extract::{DefaultBodyLimit, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use inna_core::{
    client::InnaClient,
    session::{SessionCookie, StoredSession},
};
use serde::Deserialize;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::sync::{oneshot, watch};

const TIMEOUT: Duration = Duration::from_secs(600);

pub struct BrowserAuthHandle {
    pub url: String,
    receiver: oneshot::Receiver<StoredSession>,
    stop: watch::Sender<bool>,
}

impl Drop for BrowserAuthHandle {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
    }
}

impl BrowserAuthHandle {
    pub async fn wait(mut self) -> Result<StoredSession, String> {
        tokio::time::timeout(TIMEOUT, &mut self.receiver)
            .await
            .map_err(|_| {
                "Browser connection expired. Choose Connect in browser to try again.".to_string()
            })?
            .map_err(|_| "Browser connection stopped. Please try again.".to_string())
    }
}

struct Handoff {
    token: String,
    host: String,
    done: AtomicBool,
    session: Mutex<Option<oneshot::Sender<StoredSession>>>,
    validating: tokio::sync::Mutex<()>,
}

pub fn extension_dir() -> PathBuf {
    let installed = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|parent| parent.join("browser-extension")));
    installed
        .filter(|path| path.join("manifest.json").is_file())
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../browser-extension")
        })
}

pub async fn start() -> Result<BrowserAuthHandle, String> {
    let mut random = [0u8; 32];
    getrandom::getrandom(&mut random)
        .map_err(|_| "Could not create a secure browser connection.".to_string())?;
    let token: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|_| "Could not start the browser connection on this computer.".to_string())?;
    let host = listener
        .local_addr()
        .map_err(|_| "Could not read the browser connection address.".to_string())?
        .to_string();
    let url = format!("http://{host}/connect#{token}");
    let (tx, receiver) = oneshot::channel();
    let state = Arc::new(Handoff {
        token,
        host,
        done: AtomicBool::new(false),
        session: Mutex::new(Some(tx)),
        validating: tokio::sync::Mutex::new(()),
    });
    let router = Router::new()
        .route("/connect", get(landing))
        .route("/connect.js", get(script))
        .route("/info", get(info))
        .route("/status", get(status))
        .route("/session", post(receive))
        .layer(DefaultBodyLimit::max(128 * 1024))
        .with_state(state);
    let (stop, mut stopped) = watch::channel(false);
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).with_graceful_shutdown(async move {
            tokio::select! { _ = stopped.changed() => {}, _ = tokio::time::sleep(TIMEOUT) => {} }
        }).await;
    });
    Ok(BrowserAuthHandle {
        url,
        receiver,
        stop,
    })
}

fn authorized(state: &Handoff, headers: &HeaderMap) -> bool {
    headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        == Some(state.host.as_str())
        && headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .is_some_and(|token| {
                token.len() == state.token.len()
                    && token
                        .bytes()
                        .zip(state.token.bytes())
                        .fold(0u8, |diff, (a, b)| diff | (a ^ b))
                        == 0
            })
}

async fn landing(State(state): State<Arc<Handoff>>, headers: HeaderMap) -> Response {
    if headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        != Some(state.host.as_str())
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let path = extension_dir()
        .display()
        .to_string()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;");
    let html =
        include_str!("../assets/browser-connect.html").replace("{{EXTENSION_FOLDER}}", &path);
    let mut response = Html(html).into_response();
    response.headers_mut().insert(header::CONTENT_SECURITY_POLICY,
        "default-src 'none'; style-src 'unsafe-inline'; script-src 'self'; connect-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'".parse().unwrap());
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response
        .headers_mut()
        .insert(header::REFERRER_POLICY, "no-referrer".parse().unwrap());
    response
}

async fn script() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/javascript"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        include_str!("../assets/browser-connect.js"),
    )
}

async fn info(State(state): State<Arc<Handoff>>, headers: HeaderMap) -> Response {
    if !authorized(&state, &headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    Json(serde_json::json!({"protocol": "anni-browser-connect-v1"})).into_response()
}

async fn status(State(state): State<Arc<Handoff>>, headers: HeaderMap) -> Response {
    if !authorized(&state, &headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    Json(serde_json::json!({"connected": state.done.load(Ordering::SeqCst)})).into_response()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Incoming {
    cookies: Vec<SessionCookie>,
}

fn valid_cookies(cookies: &[SessionCookie]) -> bool {
    !cookies.is_empty()
        && cookies.len() <= 64
        && cookies.iter().all(|cookie| {
            matches!(
                cookie.domain.trim_start_matches('.'),
                "nam.inna.is" | "inna.is"
            ) && !cookie.name.is_empty()
                && cookie.name.len() <= 256
                && cookie
                    .name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
                && cookie.value.len() <= 8192
                && !cookie
                    .value
                    .chars()
                    .any(|letter| letter.is_control() || letter == ';')
                && cookie.path.starts_with('/')
                && cookie.path.len() <= 256
                && !cookie.path.chars().any(char::is_control)
        })
}

async fn receive(
    State(state): State<Arc<Handoff>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> StatusCode {
    if !authorized(&state, &headers) {
        return StatusCode::UNAUTHORIZED;
    }
    let Ok(incoming) = serde_json::from_slice::<Incoming>(&body) else {
        return StatusCode::BAD_REQUEST;
    };
    if !valid_cookies(&incoming.cookies) {
        return StatusCode::BAD_REQUEST;
    }
    let _validation = state.validating.lock().await;
    if state.done.load(Ordering::SeqCst) {
        return StatusCode::GONE;
    }
    let session = StoredSession {
        cookies: incoming.cookies,
        saved_at: chrono::Utc::now().to_rfc3339(),
    };
    let Ok(client) = InnaClient::new() else {
        return StatusCode::SERVICE_UNAVAILABLE;
    };
    client.load_session(&session);
    // Validate against INNA before accepting anything into the app's saved account.
    if client.current_user().await.is_err() {
        return StatusCode::UNPROCESSABLE_ENTITY;
    }
    let Some(sender) = state.session.lock().unwrap().take() else {
        return StatusCode::GONE;
    };
    state.done.store(true, Ordering::SeqCst);
    if sender.send(session).is_err() {
        return StatusCode::GONE;
    }
    StatusCode::NO_CONTENT
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn handoff_requires_random_capability_and_does_not_expose_it_in_page() {
        let first = start().await.unwrap();
        let second = start().await.unwrap();
        let (base, token) = first.url.split_once('#').unwrap();
        assert_eq!(token.len(), 64);
        assert_ne!(
            first.url.split_once('#').unwrap().1,
            second.url.split_once('#').unwrap().1
        );
        let root = base.trim_end_matches("/connect");
        let client = reqwest::Client::new();
        let page = client.get(base).send().await.unwrap().text().await.unwrap();
        assert!(!page.contains(token));
        assert_eq!(
            client
                .get(format!("{root}/info"))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            client
                .get(format!("{root}/info"))
                .bearer_auth(token)
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        assert_eq!(
            client
                .post(format!("{root}/session"))
                .json(&serde_json::json!({"cookies": []}))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            client
                .post(format!("{root}/session"))
                .bearer_auth(token)
                .json(&serde_json::json!({"cookies": []}))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            client
                .get(base)
                .header(header::HOST, "attacker.example")
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::BAD_REQUEST
        );
        drop(first);
        drop(second);
    }

    #[test]
    fn handoff_rejects_google_cookies_and_header_injection() {
        let mut cookie = SessionCookie {
            name: "session".into(),
            value: "example".into(),
            domain: ".inna.is".into(),
            path: "/".into(),
            http_only: true,
            secure: true,
        };
        assert!(valid_cookies(&[cookie.clone()]));
        cookie.domain = ".google.com".into();
        assert!(!valid_cookies(&[cookie.clone()]));
        cookie.domain = "evilinna.is".into();
        assert!(!valid_cookies(&[cookie.clone()]));
        cookie.domain = "nam.inna.is".into();
        cookie.value = "x\r\nCookie: other".into();
        assert!(!valid_cookies(&[cookie]));
    }
}
