# Testing the Rust+ Companion App

This document outlines the procedures for verifying the end-to-end (E2E) flow of the Nodirust Companion App.

The complete flow involves multiple moving parts that communicate with external APIs (Facepunch, Expo, Firebase Cloud Messaging). 

## End-to-End Flow Validation

The application requires passing through several external steps to be fully authenticated:

1. **FCM Device Registration:** Generating a generic Android Firebase Cloud Messaging token.
2. **Expo Push Registration:** Exchanging the FCM token for an Expo push token.
3. **Steam Authentication:** Capturing the Facepunch AuthToken via a Steam login popup.
4. **Rust+ API Pairing:** Submitting the Expo token and Facepunch AuthToken to `companion-rust.facepunch.com/api/push/register` to bind the device for push notifications.

### Note on Registration Flow (from CLI Prototype)

The Facepunch companion app is built on Expo (React Native). As such, Facepunch expects an **Expo Push Token**, not a raw FCM or WebPush token. The correct flow is:

1. **FCM Android Registration:** Use the generic Firebase Cloud Messaging API to register as an Android device.
   - Project ID: `rust-companion-app`
   - Sender ID: `976529667804`
   - App ID: `1:976529667804:android:d6f1ddeb4403b338fea619`
   - Package Name: `com.facepunch.rust.companion`
2. **Expo Token Exchange:** Send the raw FCM token to `https://exp.host/--/api/v2/push/getExpoPushToken`.
   - `projectId`: `49451aca-a822-41e6-ad59-955718d0ff9c`
3. **Facepunch Registration:** Send the Expo Push Token (along with the Steam Auth Token) to Facepunch via `https://companion-rust.facepunch.com:443/api/push/register` with `push_kind: 3` (Expo).

### 1. Manual Testing

Because the real Steam login requires interactive user input (and often 2FA/Captchas), automated integration testing of the actual production endpoint is not possible. You can manually test the flow by running the app.

1. **Launch the Application:**
   Run `cargo run -p app`. The system tray icon should appear.
2. **Trigger the Login Flow:**
   Click the tray icon -> Click **Settings**. In the settings dashboard, click the "Login with Steam" button.
3. **Authenticate:**
   The native `wry` webview will spawn, taking you to the Facepunch Steam login. Complete the login. The webview should automatically close once the `ReactNativeWebView.postMessage` payload is intercepted.
4. **Verify Persistence:**
   Check your OS credential manager for the `nodirust_app` service with the `steam_token` key.
5. **Verify Pairing:**
   The app will automatically register your FCM token with the Facepunch API, meaning your `config.json` (in the OS AppData directory) should start populating with `ServerConfig` details the next time you connect to a Rust server in-game.

### 2. Simulating the Flow (For CLI/Integration Tools)

If you need to build CLI-based integration tests without relying on the native `wry` webview, you can use a local loopback server to catch the callback from a normal browser, just like in the original CLI implementation.

A mock test flow can be implemented as follows:
- Spawn a local HTTP server (e.g., using `axum` on `localhost:3000`).
- Serve an HTML page that opens `https://companion-rust.facepunch.com/login` and injects the `ReactNativeWebView.postMessage` bridge mock.
- Programmatically launch Google Chrome / Edge pointing to the local server with `--disable-web-security` and `--disable-site-isolation-trials` to allow the popup cross-origin bridge.
- Wait for the local server to receive the `/callback?token=...` request.
- Run `push_receiver` and `facepunch::FacepunchClient` registration logic.

### 3. Automated Protocol Tests

For unit testing `rustplus` Protocol buffers and websocket parsing without hitting real Facepunch servers:

- **Mock the WebSocket:** In `crates/rustplus/tests/`, you can use `tokio-tungstenite` to spin up a mock WebSocket server on `127.0.0.1:0`. Connect the `RustPlusClient` to it and send binary Protobuf payloads (`AppMessage`) to verify that `AppBroadcast` events (like `AppNewTeamMessage`, `AppEntityChanged`) are parsed correctly.
- **Mock the FCM Stream:** In `crates/push_receiver/tests/`, spin up a mock TLS server that streams Google MCS (Mobile Connection Server) Protocol Buffer handshake bytes.

## Checking the Native Webview Auth Integration

In the `app` crate, the native webview login logic handles the Facepunch Steam login process cleanly:

```rust
// Located in app/src/ui/auth.rs
let initialization_script = r#"
    window.ReactNativeWebView = {
        postMessage: function(msg) {
            window.ipc.postMessage(msg);
        }
    };
"#;
```

This intercepts the exact JSON payload expected by the Facepunch mobile app and pipes it securely through the OS-native Rust IPC bridge, avoiding local web servers and cross-origin browser hacks entirely.
