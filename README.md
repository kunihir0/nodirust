# NODIrust

A lightweight background daemon for Rust+ smart alarms. Runs silently in your system tray and pushes native OS notifications directly to your desktop. macOS & Windows only.

## Performance

Built purely in Rust without heavy browser wrappers. Optimized for the background.
- **RAM:** ~2.5 MB
- **CPU:** 0.0% idle
- **Binary Size:** < 20 MB (compresses to ~4MB via UPX)

## Architecture

The binary splits into two processes:
1. **Daemon:** A headless background process managing the async Tokio runtime and FCM notifications.
2. **UI:** An ephemeral GUI process (system tray, settings window) that opens only on demand and is destroyed when closed.

## Privacy & Security

NODIrust collects **zero** analytics or telemetry.

**Steam Authentication:**
We do not touch or see your Steam password. NODIrust uses an ephemeral OS webview to load the official Facepunch login portal. It intercepts the generated Facepunch token, saves it securely in your local AppData folder, and immediately destroys the window.

To prevent leaks, the configuration structs intentionally omit the `Debug` trait so tokens physically cannot be logged. The token is never broadcast over local IPC.

*Audit the token capture logic at: `crates/app/src/ui/auth.rs`*

## Build

```bash
cargo build --release
```
Run the executable and look for the NODIrust icon in your system tray.
