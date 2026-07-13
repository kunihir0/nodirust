# Rust+ Companion App

> New project — this file defines the target architecture and standards from day one, not a retrofit onto existing code.

## 1. Project Overview

A lightweight, cross-platform (Windows + macOS) native desktop utility that securely connects to Rust+ (Facepunch's companion protocol for the game Rust) game servers and monitors in-game events — without the overhead of a heavy web framework like Tauri or Electron.

Core responsibilities:

- **Authentication & Server Pairing** — drive the Facepunch Steam login flow through a temporary, secure native window to authenticate the user, then pair with specific Rust servers.
- **Background Monitoring** — run silently in the system tray, maintaining an active, asynchronous connection to paired game servers without needing a persistent UI window open.
- **Real-Time Alerts** — listen for in-game smart-alarm triggers (such as "boom" alerts or raid notifications) and instantly push native OS notifications to the desktop.

**North star:** a low-latency, low-memory, high-priority, small-footprint background listener that keeps you informed of critical in-game events across your paired servers. Memory footprint gets called out specifically because it's a hard constraint here, not a preference — this thing should be able to sit in the tray all day and never be the reason someone's fans spin up. Every architecture and dependency decision below answers to that.

**Non-negotiables**, stated up front because they shape everything else in this document:

- Windows and macOS only. There is no Linux target (§4).
- No JavaScript, no Node.js/npm toolchain, no bundled browser engine beyond the one ephemeral login webview described in §5.
- Dependencies are minimized aggressively. Reach for the standard library first, and justify anything that isn't `rustplus` or `push_receiver` — which we own (§6).

## 2. Architecture & Library Stack

### First-Party Crates (Ours — Not Vendored)

These are our own crates, pulled into the workspace as path dependencies (§3). They're held to the exact same standards as everything else in this document — agents should read, extend, and fix them directly, not treat them as opaque black boxes.

- **`rustplus`** — our own implementation of the Rust+ smart-device protocol: protobuf message definitions, the WebSocket client, and the request/response plumbing for talking to a paired Rust server. Not a wrapper around a third-party `rustplus` crate — ours end to end.
- **`push_receiver`** — our own implementation of an FCM (Firebase Cloud Messaging) push receiver, used to receive the push-triggered events (like smart alarm triggers) that Facepunch's backend sends the way it would to the official mobile app — without pulling in the Firebase SDK.

Because we own both, protobuf codegen inside `rustplus` (e.g. `prost` + `prost-build`) and the FCM/MCS protocol details inside `push_receiver` are implementation details private to those crates. The rest of the workspace depends on their public API, not on how they're built internally.

### UI & Windowing

- **UI Framework:** `egui` (via `eframe`)
  *Rationale:* Immediate-mode, no web dependency, compiles to a small native binary — well suited to a utility that's mostly a tray icon with an occasional settings window.
- **System Tray:** `tray-icon` (alongside `winit`/`tao`)
  *Rationale:* Lets the app live silently in the Windows/macOS system tray without a persistent open window.

### Authentication

- **Login Window:** `wry` & `winit`
  *Rationale:* An ephemeral, native webview used strictly to render Facepunch's Steam login page, intercept its `window.ReactNativeWebView.postMessage` payload via wry's native IPC handler, and close itself the instant a token is captured. See §5 (Native-Only Policy) for exactly how far this goes and no further.

### OS Integration

- **Push Notifications:** `notify-rust`
  *Rationale:* Talks directly to each platform's native notification center — WinRT on Windows, `UNUserNotificationCenter` on macOS — to surface "boom"/raid alerts. No D-Bus/XDG path; see §4 (Platform Scope).

### Async Runtime & Networking

- **Async Runtime:** `tokio`, with targeted features rather than `"full"` (§6)
  *Rationale:* Powers the background listener(s) for the `rustplus` WebSocket connection and the `push_receiver` stream while the UI is idle or closed.
- **HTTP Client:** `reqwest`, targeted features (§6)
  *Rationale:* FCM registration, Expo token exchange, and Facepunch API calls.
- **Serialization:** `serde`, `serde_json`
  *Rationale:* JSON payloads during auth callbacks and API requests.

## 3. Workspace Layout

```
NODIRUST/
├── Cargo.toml                  # [workspace] manifest + shared profile settings
├── AGENTS.md
└── crates/
    ├── app/                    # binary: egui UI, tray, wiring, login window
    │   ├── src/main.rs         # wiring + top-level execution ONLY — no business logic
    │   └── tests/
    ├── rustplus/                # OWNED — Rust+ protocol client
    │   ├── src/
    │   └── tests/
    └── push_receiver/           # OWNED — FCM push receiver
        ├── src/
        └── tests/
```

- `app` depends on `rustplus` and `push_receiver` as path dependencies and contains no protocol or push logic of its own — it wires them together, draws the UI, and owns the tray/notification glue.
- `rustplus` and `push_receiver` are runtime-agnostic library crates (§8) and must build and be independently testable without `app`.
- A new cross-cutting concern (e.g., credential storage, §12) gets its own crate under `crates/` once it's more than a couple of files — it doesn't grow inside `app`.

This is a starting layout for a project that hasn't been scaffolded yet; adjust as real needs surface, but keep the underlying rule — protocol and push logic live in owned library crates, `app` only wires them up — intact.

## 4. Platform Scope

Target platforms: Windows and macOS only. There is no Linux target for this project, and that has concrete effects on library choices:

- `notify-rust` — only the WinRT (Windows) and `UNUserNotificationCenter` (macOS) backends matter. Don't add or maintain a D-Bus/XDG code path.
- `tray-icon` — only the Windows and macOS backends need to work; X11/Wayland tray protocols aren't a concern.
- Any `cfg(target_os = "...")` branch for an OS outside `{windows, macos}` is dead code — don't write it.

If Linux support becomes a goal later, it'll be scoped as a deliberate follow-up, not something that falls out of "it happened to compile."

## 5. Native-Only Policy (No JavaScript, No Node Toolchain)

This project has zero authored JavaScript or TypeScript, and no Node.js/npm/yarn/pnpm/webpack/vite tooling anywhere in the build. Every line of application logic — auth handling, protocol parsing, UI, tray, notifications — is Rust.

The one thing that looks like an exception: Facepunch's Steam login page is a web page that they control, and the only way to authenticate against it is to render it. `wry` gives us a disposable, OS-native webview for exactly that one page and nothing more:

- The webview's only job is displaying Facepunch's login page and closing itself the instant a token is captured. It's never a general-purpose UI surface, and no other page is ever loaded into it.
- Bridging the page's `window.ReactNativeWebView.postMessage(...)` call — it expects to talk to the official mobile app's bridge — into wry's IPC handler may require a tiny initialization script. That script lives as a short string literal in Rust source, never as a standalone `.js` file, asset, or build step, and does nothing but forward the raw message to the native IPC handler. All parsing, validation, and handling of the payload happens in Rust on the other side.
- No other web content, local HTML files, or JS bridges are permitted anywhere else in the app.

## 6. Dependency Philosophy (Read Before `cargo add`)

Minimal footprint is a first-class requirement here, not a nice-to-have — it's a large part of why this app exists instead of "just use the Electron one."

- **Standard library first.** Before reaching for a crate, check whether `std` already solves the problem. For an app this size, it usually does.
- **Every dependency is a decision, not a default.** When adding one, note what it's for, why `std` or an existing dependency can't do it, and what it pulls in transitively (`cargo tree -p <crate>`).
- **Prefer small, focused crates over "kitchen sink" ones.** A crate with two transitive dependencies that does one thing beats a framework that does ten things to get the one you need.
- **Turn off default features you don't use.** `default-features = false`, then opt in explicitly. `tokio` is the running example: enable the specific features actually used (`rt-multi-thread`, `macros`, `net`, `time`, `sync`, …) instead of `features = ["full"]`.
- **Audit periodically.** `cargo tree --duplicates` catches multiple versions of the same crate pulled in transitively; consider `cargo-machete` (or similar) in CI to catch declared-but-unused dependencies.
- **No JS/Node toolchain, ever** — see §5. This includes dev-dependencies used only for tooling.
- **`eframe`/`egui` and `wry` are accepted exceptions, not a precedent.** A native GUI and a login webview are the reason this app exists, so their transitive graphics/windowing stacks are an accepted cost. That doesn't make "we already have a big dependency tree, one more won't matter" a valid argument for the next addition.

## 7. Standards for AI Coding Agents

You are acting as a senior Rust engineer on this codebase. Write idiomatic, production-grade Rust — no shortcuts, and don't fight the borrow checker, work with it. When a crate's API isn't something you're certain of — especially fast-moving ones like `egui`, `wry`, or `tokio` — pull current docs via Context7 (or equivalent) rather than trusting memorized training data, which is a real source of subtly-wrong code for crates that move this fast.

The rules below are absolute and apply to every crate in the workspace, including `rustplus` and `push_receiver`. They're ours, not vendored black boxes, so the same bar applies to them as to `app`.

- **Formatting:** `cargo fmt --all` before every commit. Enforced in CI (§14).
- **Linting:** `cargo clippy --workspace --all-targets -- -D warnings -D clippy::pedantic -D clippy::cargo` must be clean. Enforced in CI (§14).
- **Error Handling (Library Code):** Zero `unwrap()` in library code. Zero.
- **Error Handling (General):** Prefer `?` or `let-else` to handle errors as data. Reserve `unwrap()` only for tests, and `unwrap_or_else()` only for inline defaults.
- **Documentation:** Every public item in a library crate has a `///` doc comment.
- **Version Control:** `Cargo.lock` is committed — this is an application workspace.

## 8. Async & Concurrency

- **Runtime:** The async runtime is `tokio`. Library crates must not call `#[tokio::main]` or `Runtime::new` — only `app`'s `main.rs` owns the runtime.
- **Runtime-Agnostic Libraries:** Library async functions are runtime-agnostic — they use `async fn` and `.await`, nothing more.
- **Structured Concurrency:** Prefer `tokio::select!` for racing futures. Use `JoinSet` for structured concurrency over raw `spawn`.

```rust
// Wrong — spawns unstructured, errors are lost
tokio::spawn(do_thing());

// Right — structured, errors surface
let mut set = JoinSet::new();
set.spawn(do_thing());
while let Some(res) = set.join_next().await {
    res??;
}
```

- **Mutexes:** Do not hold a `std::sync::MutexGuard` across an `.await` point. Use `tokio::sync::Mutex` where async locking is needed, `std::sync::Mutex` everywhere else.
- **Necessity:** If a function is not async, don't make it async.
- **Latency budget:** the path from an incoming event — a `rustplus` WebSocket message or a `push_receiver` payload — to an OS notification being dispatched is the most latency-sensitive path in the app. Don't add buffering, polling, or unnecessary task hops on this path.
- **Bounded channels:** event queues use bounded `tokio::sync::mpsc` (or similar), not unbounded ones. Backpressure should surface as an explicit error/log, never as silently growing memory.
- **Notification priority:** dispatch alarm notifications at the highest priority/interruption level the platform's notification API exposes. A raid alarm is not an FYI — it shouldn't get silently swallowed by a Focus/Do Not Disturb mode where the platform allows an override.

## 9. Ownership & Types

- **References:** Prefer `&str` over `&String`, and `&[T]` over `&Vec<T>` in function signatures.
- **Constructors:** Use `impl Into<String>` for owned string params in constructors.
- **Shared Ownership:** Reach for `Arc<T>` only when shared ownership is genuinely needed. If you find yourself cloning `Arc` everywhere, rethink the data model.
- **Newtypes:** Use the newtype pattern over type aliases for domain types (e.g., `struct UserId(u64)` not `type UserId = u64`).
- **Type State:** Make illegal states unrepresentable. Encode invariants in the type system, not runtime checks.

```rust
// Don't do this
fn process(status: String) { ... }

// Do this
enum Status { Active, Inactive }
fn process(status: Status) { ... }
```

## 10. Structs, Builders & Traits

- **Builder Pattern:** For structs with more than ~3 optional fields, use the builder pattern:

```rust
pub struct PushReceiver {
    sender_id: String,
    http: reqwest::Client,
    timeout: Duration,
}

impl PushReceiver {
    pub fn builder(sender_id: impl Into<String>) -> PushReceiverBuilder {
        PushReceiverBuilder::new(sender_id)
    }
}
```

- **Derives:** Derive `Debug` on every struct and enum — except types holding a token, key, or pairing secret, which get a hand-written `Debug` impl that redacts the sensitive field(s) (e.g., printing `"<redacted>"` in their place) instead of a blind derive. See §12 (Secrets & Local State).
- Derive `Clone` when it makes sense. Derive `Copy` only for small value types.
- **Standard Traits:** Implement std traits where appropriate: `Display`, `From`, `TryFrom`, `Iterator`, `Default`.
- **Polymorphism:** Prefer `impl Trait` in return position for single concrete types. Use `dyn Trait` only when you genuinely need runtime polymorphism.
- **Trait Design:** Keep traits small and focused — prefer multiple small traits over one large one.

## 11. Logging

- **Framework:** All crates use `tracing`. Never use `println!` or `eprintln!` for diagnostics.
- **Structured Logging:**

```rust
// Wrong
println!("Connected: {}", id);

// Right — structured fields, not string formatting
tracing::info!(connection_id = %id, "Connected");

// Spans for operations with duration
let _span = tracing::info_span!("register", sender_id = %self.sender_id).entered();
```

- **Configuration:** Library crates only emit spans/events. Only the binary configures the subscriber.
- **Secrets:** Never log a raw token or pairing credential — same redaction rule as §10's `Debug` exception. Log that a token was received or refreshed, not its value.

## 12. Secrets & Local State

The app persists a Steam-derived auth token and per-server pairing data (address, port, player token) between runs. Treat all of it as sensitive:

- Never derive `Debug` blindly on a type holding one of these (§10), and never let one reach a tracing log line (§11).
- Local state lives outside version control — `.gitignore` it — and outside the repo/workspace directory entirely, in an OS-appropriate app-data location (resolved via a small, single-purpose path crate rather than hand-rolled logic; evaluate any such crate against §6 like anything else).
- Whether the auth token belongs in a flat file in that app-data directory or in the OS keychain is an open decision. Weigh a keychain-access crate against §6 before reaching for one — don't add it "just in case."
- Never commit a real token, pairing file, or sample config containing one to the repository.

## 13. Testing

- **Location:** Unit tests go in the same file as the code. Integration tests go in `crates/<name>/tests/` — e.g., `crates/rustplus/tests/`, `crates/push_receiver/tests/`.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_message() { ... }

    #[tokio::test]
    async fn register_fails_without_credentials() { ... }
}
```

- **Scope:** Test the public API, not internals.
- **Async Testing:** Use `tokio::test` for async tests, not a hand-rolled runtime.
- **Naming:** Name tests as sentences: `parses_valid_message`, `returns_error_on_timeout`.
- **Mocking:** Don't mock what you don't own. Wrap external clients behind a trait and mock the trait — this is exactly how `rustplus`'s WebSocket connection and `push_receiver`'s FCM/MCS connection should be tested, since both talk to real external services.

## 14. Continuous Integration

CI is what actually enforces §7 and §6 — a rule that isn't checked in CI will rot. At minimum, CI runs on both target platforms (Windows, macOS) and fails on:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings -D clippy::pedantic -D clippy::cargo`
- `cargo test --workspace`
- (recommended) `cargo tree --duplicates`, and `cargo-machete` or similar for unused-dependency hygiene

Because this ships on both Windows and macOS, "it compiles on my machine" doesn't clear review — CI covering both platforms is what actually gates a merge.

## 15. Anti-Patterns (What Not To Do)

- **Borrow Checker:** Don't clone simply to satisfy the borrow checker. Understand why the borrow fails first and restructure the data flow if necessary.
- **Safety:** Don't reach for `unsafe` until you have definitively exhausted all safe alternatives.
- **Concurrency:** Don't use `std::sync::Mutex` inside async code — use `tokio::sync::Mutex` to avoid blocking the executor thread.
- **Application Structure:** Don't put business logic in `main.rs`. The main file should only wire things together and handle top-level execution.
- **Dependencies:** Don't add one for functionality the standard library already provides well — see §6.
- **Platform Scope:** Don't add code paths for platforms outside `{windows, macos}` — see §4.
- **JavaScript:** Don't add a JS file, a Node dependency, or any web content beyond the single login page in §5.
- **Syntax:** Don't write `return x;` at the end of a function. Use expression syntax (`x`).
- **Linting:** Don't suppress clippy lints without a comment explicitly explaining why the suppression is necessary.

## 16. Comments Policy

- **Intent over Action:** Only write comments that explain *why* a non-obvious decision was made, not *what* the code does.
- **No TODOs:** Do not litter the codebase with TODO comments. Track outstanding work and technical debt in dedicated issue trackers.