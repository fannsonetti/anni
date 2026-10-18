mod common;
mod phone;

use clap::Parser;
use common::CHECK_URL;
use inna_core::endpoints::INNA_BASE_URL;
use inna_core::session::{SessionCookie, StoredSession};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

#[derive(Parser)]
#[command(name = "inna-auth", about = "INNA authentication helper")]
struct Cli {
    /// Phone QR login over LAN (for devices that cannot open a browser)
    #[arg(long)]
    phone: bool,
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("inna_auth=info")
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    if cli.phone {
        tracing::info!("starting phone QR authentication");
        phone::run_phone_login();
    }

    tracing::info!("starting local WebView authentication");
    run_local_login();
}

fn run_local_login() {
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();

    let window = WindowBuilder::new()
        .with_title("anni · Sign in to INNA")
        .with_inner_size(tao::dpi::LogicalSize::new(1024.0, 700.0))
        .build(&event_loop)
        .expect("failed to create window");

    let proxy = event_loop.create_proxy();
    let check_url = format!("{INNA_BASE_URL}{CHECK_URL}");

    let poll_script = format!(
        r#"
        (function() {{
            if (window.__inna_polling) return;
            window.__inna_polling = true;
            async function check() {{
                try {{
                    let r = await fetch("{check_url}", {{
                        credentials: "include",
                        headers: {{ "X-Requested-By": "XMLHttpRequest" }}
                    }});
                    if (r.status === 200) {{
                        let data = await r.json();
                        if (data && data.studentName) {{
                            window.ipc.postMessage("AUTH_OK");
                            return;
                        }}
                    }}
                }} catch(e) {{}}
                setTimeout(check, 2000);
            }}
            setTimeout(check, 1500);
        }})();
        "#
    );

    let webview = WebViewBuilder::new()
        .with_url(INNA_BASE_URL)
        .with_initialization_script(&poll_script)
        .with_ipc_handler(move |msg| {
            let body = msg.body();
            if body == "AUTH_OK" {
                let _ = proxy.send_event(UserEvent::AuthCompleted);
            }
        })
        .build(&window)
        .expect("failed to create webview");

    event_loop.run(move |event, _target, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::UserEvent(UserEvent::AuthCompleted) => {
                tracing::info!("auth signal received, extracting cookies");
                match extract_session(&webview) {
                    Ok(session) => {
                        let json = serde_json::to_string(&session).expect("serialize session");
                        println!("{json}");
                        tracing::info!("authentication completed successfully");
                        std::process::exit(0);
                    }
                    Err(e) => {
                        tracing::error!("cookie extraction failed: {e}");
                        std::process::exit(1);
                    }
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                tracing::error!("login window closed by user");
                std::process::exit(1);
            }
            _ => {}
        }
    });
}

fn extract_session(webview: &wry::WebView) -> Result<StoredSession, String> {
    let inna_url = format!("{INNA_BASE_URL}/");
    let cookies = webview
        .cookies_for_url(&inna_url)
        .map_err(|e| format!("failed to get cookies: {e}"))?;

    if cookies.is_empty() {
        return Err("no cookies found for INNA URL".into());
    }

    tracing::info!("extracted {} cookies", cookies.len());

    let session_cookies: Vec<SessionCookie> = cookies
        .iter()
        .map(|c| SessionCookie {
            name: c.name().to_string(),
            value: c.value().to_string(),
            domain: c.domain().unwrap_or("nam.inna.is").to_string(),
            path: c.path().unwrap_or("/").to_string(),
            http_only: c.http_only().unwrap_or(false),
            secure: c.secure().unwrap_or(false),
        })
        .collect();

    Ok(StoredSession {
        cookies: session_cookies,
        saved_at: chrono::Utc::now().to_rfc3339(),
    })
}

#[derive(Debug)]
enum UserEvent {
    AuthCompleted,
}
