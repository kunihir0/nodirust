use notify_rust::Notification;

#[cfg(target_os = "windows")]
const WINDOWS_APP_ID: &str = "kunihir0.NODIrust";

#[derive(Debug, thiserror::Error)]
pub enum NotificationError {
    #[cfg(target_os = "windows")]
    #[error("failed to register the Windows notification identity: {0}")]
    WindowsIdentity(String),
    #[error("the operating system rejected the notification: {0}")]
    Delivery(#[from] notify_rust::error::Error),
}

pub struct Notifier;

impl Notifier {
    /// Submits a native notification and keeps its activation handler alive.
    ///
    /// # Errors
    ///
    /// Returns an error when the platform identity cannot be registered or the
    /// operating system rejects the notification request.
    pub fn push(title: &str, body: &str) -> Result<(), NotificationError> {
        #[cfg(target_os = "windows")]
        ensure_windows_identity()?;

        let mut notification = Notification::new();
        notification.summary(title).body(body);

        #[cfg(target_os = "windows")]
        notification.app_id(WINDOWS_APP_ID);

        let handle = notification.show()?;
        std::thread::spawn(move || {
            if let Err(error) = handle.wait_for_response(handle_notification_response) {
                tracing::warn!(%error, "Notification response handler stopped");
            }
        });
        Ok(())
    }
}

fn handle_notification_response(response: &notify_rust::NotificationResponse) {
    if !matches!(*response, notify_rust::NotificationResponse::Default) {
        return;
    }
    let Ok(executable) = std::env::current_exe() else {
        tracing::error!("Could not locate NODIrust after notification activation");
        return;
    };
    if let Err(error) = std::process::Command::new(executable).arg("--ui").spawn() {
        tracing::error!(%error, "Failed to open NODIrust from notification");
    }
}

#[cfg(target_os = "windows")]
fn ensure_windows_identity() -> Result<(), NotificationError> {
    static REGISTRATION: std::sync::OnceLock<Result<(), String>> = std::sync::OnceLock::new();
    REGISTRATION
        .get_or_init(register_windows_identity)
        .clone()
        .map_err(NotificationError::WindowsIdentity)
}

#[cfg(target_os = "windows")]
fn register_windows_identity() -> Result<(), String> {
    let key_path = format!(r"SOFTWARE\Classes\AppUserModelId\{WINDOWS_APP_ID}");
    let key = windows_registry::CURRENT_USER
        .create(key_path)
        .map_err(|error| error.to_string())?;
    key.set_string("DisplayName", "NODIrust")
        .map_err(|error| error.to_string())?;
    key.set_string("IconBackgroundColor", "0")
        .map_err(|error| error.to_string())?;

    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let icon_uri: windows_registry::HSTRING = executable.as_path().into();
    key.set_hstring("IconUri", &icon_uri)
        .map_err(|error| error.to_string())
}
