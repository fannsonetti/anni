#[cfg(windows)]
#[test]
#[ignore = "opens temporary synthetic WebViews in child processes"]
fn remembered_profile_survives_helper_restart() {
    let root =
        std::env::temp_dir().join(format!("anni-webview-profile-test-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    for phase in ["write", "read"] {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "tests::native_profile_child",
                "--ignored",
                "--test-threads=1",
            ])
            .env("ANNI_TEST_SIGN_IN_PROFILE", &root)
            .env("ANNI_TEST_SIGN_IN_PHASE", phase)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while child.try_wait().unwrap().is_none() {
            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("Native WebView test timed out in {phase}; run it on an interactive Windows desktop");
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{phase} helper failed: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        // Let the native browser finish closing before launching another helper.
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    assert!(root.starts_with(std::env::temp_dir()));
    for attempt in 0..20 {
        match std::fs::remove_dir_all(&root) {
            Ok(()) => return,
            Err(error) if attempt == 19 => {
                panic!("Could not remove temporary test profile: {error}")
            }
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(100)),
        }
    }
}

#[cfg(windows)]
#[test]
#[ignore = "internal child of remembered_profile_survives_helper_restart"]
fn native_profile_child() {
    use super::*;
    use tao::platform::windows::EventLoopBuilderExtWindows;
    let Some(profile) = std::env::var_os("ANNI_TEST_SIGN_IN_PROFILE") else {
        return;
    };
    let phase = std::env::var("ANNI_TEST_SIGN_IN_PHASE").unwrap();
    let mut builder = EventLoopBuilder::<()>::with_user_event();
    builder.with_any_thread(true);
    let mut event_loop = builder.build();
    let window = WindowBuilder::new()
        .with_title("Anni remembered sign-in test")
        .with_inner_size(tao::dpi::LogicalSize::new(380.0, 160.0))
        .build(&event_loop)
        .unwrap();
    let mut context = WebContext::new(Some(profile.into()));
    let webview = WebViewBuilder::new_with_web_context(&mut context)
        .with_incognito(false)
        .with_url("about:blank")
        .with_navigation_handler(|url| url.starts_with("about:"))
        .build(&window)
        .unwrap();
    if phase == "write" {
        let cookie = wry::cookie::Cookie::parse("anni_remember_test=synthetic; Domain=anni-remember.test; Path=/; Max-Age=86400; Secure; HttpOnly").unwrap();
        webview.set_cookie(&cookie).unwrap();
    }
    let started = std::time::Instant::now();
    let mut checked = false;
    event_loop.run_return(|_, _, flow| {
        assert!(
            started.elapsed().as_secs() < 20,
            "Synthetic WebView did not finish"
        );
        *flow = ControlFlow::WaitUntil(
            std::time::Instant::now() + std::time::Duration::from_millis(100),
        );
        if !checked && started.elapsed().as_millis() >= 500 {
            let cookies = webview
                .cookies_for_url("https://anni-remember.test/")
                .unwrap();
            assert!(
                cookies
                    .iter()
                    .any(|cookie| cookie.name() == "anni_remember_test"
                        && cookie.value() == "synthetic"
                        && cookie.http_only() == Some(true)),
                "Remembered cookie missing in {phase} helper"
            );
            checked = true;
            *flow = ControlFlow::Exit;
        }
    });
    drop(webview);
    drop(context);
    drop(window);
    assert!(checked);
}
