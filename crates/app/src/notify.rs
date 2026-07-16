use notify_rust::Notification;

pub struct Notifier;

impl Notifier {
    pub fn push(title: &str, body: &str) {
        let mut notification = Notification::new();
        notification.summary(title).body(body);

        #[cfg(target_os = "windows")]
        notification.app_id("NODIrust");

        match notification.show() {
            Ok(handle) => {
                std::thread::spawn(move || {
                    #[allow(unused_must_use)]
                    handle.wait_for_response(|response: &notify_rust::NotificationResponse| {
                        if matches!(*response, notify_rust::NotificationResponse::Default) {
                            if let Ok(exe) = std::env::current_exe() {
                                std::process::Command::new(exe).arg("--ui").spawn().ok();
                            }
                        }
                    });
                });
            }
            Err(e) => {
                tracing::error!("Failed to show notification: {}", e);
            }
        }
    }
}
