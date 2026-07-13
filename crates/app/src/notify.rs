use notify_rust::Notification;

pub struct Notifier;

impl Notifier {
    pub fn push(title: &str, body: &str) -> Option<notify_rust::NotificationHandle> {
        let mut notification = Notification::new();
        notification.summary(title).body(body);
        
        #[cfg(target_os = "windows")]
        notification.appname("Rust+ Companion");

        match notification.show() {
            Ok(handle) => Some(handle),
            Err(e) => {
                tracing::error!("Failed to show notification: {}", e);
                None
            }
        }
    }
}
