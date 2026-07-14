# NODIrust

A lightweight, cross-platform native desktop utility that securely connects to Rust+ game servers and monitors in-game events in the background.

## Features

- Background system tray application for Windows and macOS.
- Authenticates securely via the official Facepunch Steam login.
- Real-time event notifications for Rust+ Smart Alarms via native OS notifications.
- Low-latency, low-memory footprint using a dedicated asynchronous runtime.

## Architecture

This workspace is composed of three crates:
- `app`: The main binary containing the system tray, UI, and background daemon orchestrator.
- `rustplus`: A protocol implementation and WebSocket client for communicating with Rust+ servers.
- `push_receiver`: An FCM client to securely receive push notifications (like Smart Alarms) directly from Facepunch's backend.

## Building

Requires Rust 1.75+ and Cargo.

```sh
cargo build --release
```

## Running

The application runs in the background. Look for the icon in your system tray (Windows) or menu bar (macOS) after starting the binary.

```sh
cargo run -p app --release
```
