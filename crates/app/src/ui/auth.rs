//! Ephemeral wry webview for Steam authentication.

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes};
use wry::{WebView, WebViewBuilder};

use winit::event_loop::EventLoopProxy;

struct AuthApp {
    // webview must be declared before window so it is dropped first!
    webview: Option<WebView>,
    window: Option<Window>,
    proxy: EventLoopProxy<()>,
}

impl AuthApp {
    fn new(proxy: EventLoopProxy<()>) -> Self {
        Self {
            webview: None,
            window: None,
            proxy,
        }
    }
}

impl ApplicationHandler<()> for AuthApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let attributes = WindowAttributes::default()
                .with_title("Steam Login")
                .with_inner_size(winit::dpi::LogicalSize::new(400.0, 600.0));

            let window = event_loop.create_window(attributes).unwrap();

            let proxy = self.proxy.clone();
            let ipc_handler = move |req: wry::http::Request<String>| {
                let msg = req.body();
                tracing::info!("Received IPC message from webview.");

                // Hardening: Strict IPC Payload Validation
                match serde_json::from_str::<serde_json::Value>(msg) {
                    Ok(parsed) => {
                        if let Some(token_str) = parsed.get("Token").and_then(|t| t.as_str()) {
                            tracing::info!("Captured valid Facepunch token! Saving and shutting down webview...");
                            if let Err(e) = crate::config::store::Store::set_steam_token(token_str) {
                                tracing::error!("Failed to save token to disk: {}", e);
                            }
                            let _ = proxy.send_event(());
                        } else {
                            tracing::warn!("Received valid JSON via IPC, but it lacked a Token string.");
                        }
                    }
                    Err(_) => {
                        tracing::warn!("Received malformed non-JSON IPC message.");
                    }
                }
            };

            let login_url = "https://companion-rust.facepunch.com/login";

            let webview = WebViewBuilder::new()
                .with_url(login_url)
                .with_ipc_handler(ipc_handler)
                .with_navigation_handler(|url: String| {
                    // Hardening: Prevent navigation to malicious sites (SSRF/Injection mitigation)
                    let allowed = url.starts_with("https://companion-rust.facepunch.com/") ||
                                  url.starts_with("https://steamcommunity.com/");
                    if !allowed {
                        tracing::warn!("Blocked webview navigation to unauthorized URL: {}", url);
                    }
                    allowed
                })
                .with_initialization_script(
                    r#"
                    window.ReactNativeWebView = {
                        postMessage: function(msg) {
                            window.ipc.postMessage(typeof msg === 'string' ? msg : JSON.stringify(msg));
                        }
                    };
                "#,
                )
                .build(&window)
                .unwrap();

            self.window = Some(window);
            self.webview = Some(webview);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        if let WindowEvent::CloseRequested = event {
            event_loop.exit();
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, _event: ()) {
        tracing::info!("User event received, exiting EventLoop.");
        // DO NOT drop the window here! It causes a panic if macOS has pending events like window_did_resign_key.
        // The event loop will exit, and the struct will be dropped safely in order.
        event_loop.exit();
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        tracing::info!("Exiting auth app, dropping webview and window safely.");
        self.webview = None;
        self.window = None;
    }
}

pub fn spawn_auth_webview() {
    let event_loop = EventLoop::with_user_event().build().unwrap();
    let proxy = event_loop.create_proxy();
    let mut app = AuthApp::new(proxy);
    event_loop.run_app(&mut app).unwrap();
}
