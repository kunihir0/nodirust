use notify_rust::Notification;

pub struct Notifier;

impl Notifier {
    pub fn push(title: &str, body: &str) {
        let mut notification = Notification::new();
        notification.summary(title).body(body);
        
        #[cfg(target_os = "windows")]
        notification.appname("Rust+ Companion");

        if let Err(e) = notification.show() {
            tracing::error!("Failed to show notification: {}", e);
        }
    }
}
