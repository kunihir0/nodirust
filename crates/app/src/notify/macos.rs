//! `UNUserNotificationCenter` backend.
//!
//! Alarm requests carry `UNNotificationSound.defaultSound` and the Time Sensitive
//! interruption level. Clicks are handled by one process-wide delegate, so no thread
//! is parked per delivered notification.

use super::{NotificationError, NotificationKind};
use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{Bool, ProtocolObject};
use objc2::{AnyThread, define_class, msg_send};
use objc2_foundation::{NSBundle, NSError, NSObject, NSObjectProtocol, NSString};
use objc2_user_notifications::{
    UNAlertStyle, UNAuthorizationOptions, UNAuthorizationStatus, UNMutableNotificationContent,
    UNNotification, UNNotificationDefaultActionIdentifier, UNNotificationInterruptionLevel,
    UNNotificationPresentationOptions, UNNotificationRequest, UNNotificationResponse,
    UNNotificationSetting, UNNotificationSettings, UNNotificationSound, UNUserNotificationCenter,
    UNUserNotificationCenterDelegate,
};
use std::cell::Cell;
use std::ptr::NonNull;
use std::sync::OnceLock;
use std::time::Duration;

const SETTINGS_TIMEOUT: Duration = Duration::from_secs(5);

/// Installs the response delegate and asks for alert + sound permission.
///
/// Must run on the main thread before the event loop starts: Apple requires the
/// delegate to be in place before launch finishes, otherwise the click that
/// launched the app is lost.
pub fn init() {
    if !has_bundle_identifier() {
        tracing::warn!(
            "NODIrust is not running from its .app bundle; macOS notifications are unavailable"
        );
        return;
    }
    let center = UNUserNotificationCenter::currentNotificationCenter();
    install_delegate(&center);
    request_authorization(&center);
}

pub fn push(kind: NotificationKind, title: &str, body: &str) -> Result<(), NotificationError> {
    if !has_bundle_identifier() {
        return Err(NotificationError::MissingBundle);
    }

    let content = UNMutableNotificationContent::new();
    content.setTitle(&NSString::from_str(title));
    content.setBody(&NSString::from_str(body));
    if kind == NotificationKind::Alarm {
        // The "Play sound" setting only permits a sound; the request has to carry one.
        content.setSound(Some(&UNNotificationSound::defaultSound()));
        if objc2::available!(macos = 12.0) {
            content.setInterruptionLevel(UNNotificationInterruptionLevel::TimeSensitive);
        }
    }

    let identifier = NSString::from_str(&uuid::Uuid::new_v4().to_string());
    let request =
        UNNotificationRequest::requestWithIdentifier_content_trigger(&identifier, &content, None);
    let completion = RcBlock::new(move |error: *mut NSError| {
        // SAFETY: macOS passes either null or an NSError that is valid for the
        // duration of the completion handler.
        if let Some(error) = unsafe { error.as_ref() } {
            tracing::error!(
                ?kind,
                error = %error.localizedDescription(),
                "macOS rejected the notification request; check System Settings > Notifications > NODIrust"
            );
        }
    });
    UNUserNotificationCenter::currentNotificationCenter()
        .addNotificationRequest_withCompletionHandler(&request, Some(&completion));
    Ok(())
}

/// Explains why an alarm submitted right now may not be seen or heard, or `None`
/// when macOS is configured to show and sound it.
pub async fn delivery_warning() -> Option<&'static str> {
    let (settings_tx, settings_rx) = tokio::sync::oneshot::channel();
    query_settings(move |settings| {
        let _ = settings_tx.send(settings);
    });
    let Ok(Ok(settings)) = tokio::time::timeout(SETTINGS_TIMEOUT, settings_rx).await else {
        tracing::warn!("Could not read macOS notification settings");
        return None;
    };
    settings.problem()
}

fn has_bundle_identifier() -> bool {
    // UNUserNotificationCenter raises an Objective-C exception without one.
    static BUNDLED: OnceLock<bool> = OnceLock::new();
    *BUNDLED.get_or_init(|| NSBundle::mainBundle().bundleIdentifier().is_some())
}

fn install_delegate(center: &UNUserNotificationCenter) {
    // The center holds its delegate weakly, so the process keeps it alive.
    static DELEGATE: OnceLock<Retained<NotificationDelegate>> = OnceLock::new();
    let delegate = DELEGATE.get_or_init(NotificationDelegate::new);
    center.setDelegate(Some(ProtocolObject::from_ref(&**delegate)));
}

fn request_authorization(center: &UNUserNotificationCenter) {
    let completion = RcBlock::new(|granted: Bool, error: *mut NSError| {
        // SAFETY: macOS passes either null or an NSError that is valid for the
        // duration of the completion handler.
        if let Some(error) = unsafe { error.as_ref() } {
            tracing::error!(
                error = %error.localizedDescription(),
                "macOS notification authorization request failed"
            );
        } else if !granted.as_bool() {
            tracing::warn!(
                "macOS notification permission was not granted; enable NODIrust in System Settings > Notifications"
            );
        }
        query_settings(|settings| settings.log());
    });
    center.requestAuthorizationWithOptions_completionHandler(
        UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
        &completion,
    );
}

fn query_settings(on_settings: impl FnOnce(DeliverySettings) + Send + 'static) {
    if !has_bundle_identifier() {
        return;
    }
    // Blocks are `Fn`; the slot lets the one-shot callback run exactly once.
    let on_settings = Cell::new(Some(on_settings));
    let completion = RcBlock::new(move |settings: NonNull<UNNotificationSettings>| {
        // SAFETY: macOS passes a non-null settings object that is valid for the
        // duration of the completion handler.
        let settings = DeliverySettings::from(unsafe { settings.as_ref() });
        if let Some(on_settings) = on_settings.take() {
            on_settings(settings);
        }
    });
    UNUserNotificationCenter::currentNotificationCenter()
        .getNotificationSettingsWithCompletionHandler(&completion);
}

fn open_ui() {
    let Ok(executable) = std::env::current_exe() else {
        tracing::error!("Could not locate NODIrust after notification activation");
        return;
    };
    if let Err(error) = std::process::Command::new(executable).arg("--ui").spawn() {
        tracing::error!(%error, "Failed to open NODIrust from notification");
    }
}

/// The subset of `UNNotificationSettings` that decides whether an alarm is seen and heard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DeliverySettings {
    authorization: UNAuthorizationStatus,
    alerts: UNNotificationSetting,
    alert_style: UNAlertStyle,
    sound: UNNotificationSetting,
    notification_center: UNNotificationSetting,
    /// `None` before macOS 12, which has no Time Sensitive level.
    time_sensitive: Option<UNNotificationSetting>,
}

impl From<&UNNotificationSettings> for DeliverySettings {
    fn from(settings: &UNNotificationSettings) -> Self {
        Self {
            authorization: settings.authorizationStatus(),
            alerts: settings.alertSetting(),
            alert_style: settings.alertStyle(),
            sound: settings.soundSetting(),
            notification_center: settings.notificationCenterSetting(),
            time_sensitive: objc2::available!(macos = 12.0)
                .then(|| settings.timeSensitiveSetting()),
        }
    }
}

impl DeliverySettings {
    fn problem(&self) -> Option<&'static str> {
        match self.authorization {
            UNAuthorizationStatus::Denied => {
                return Some(
                    "macOS is blocking NODIrust notifications. Turn on Allow Notifications in System Settings > Notifications > NODIrust.",
                );
            }
            UNAuthorizationStatus::NotDetermined => {
                return Some(
                    "macOS has not granted NODIrust notification permission yet. Restart NODIrust and allow notifications when asked.",
                );
            }
            UNAuthorizationStatus::Provisional => {
                return Some(
                    "macOS is delivering NODIrust notifications quietly. Choose Deliver Immediately for NODIrust in Notification Center or System Settings.",
                );
            }
            _ => {}
        }
        if self.alerts == UNNotificationSetting::Disabled || self.alert_style == UNAlertStyle::None
        {
            return Some(
                "Banners are off for NODIrust, so alarms only land in Notification Center. Set the alert style to Temporary or Persistent in System Settings > Notifications > NODIrust.",
            );
        }
        if self.sound == UNNotificationSetting::Disabled {
            return Some(
                "Sounds are off for NODIrust, so alarms are silent. Turn on Play sound for notification in System Settings > Notifications > NODIrust.",
            );
        }
        if self.notification_center == UNNotificationSetting::Disabled {
            return Some(
                "Notification Center is off for NODIrust, so missed alarms are not kept. Turn on Show in Notification Center in System Settings > Notifications > NODIrust.",
            );
        }
        None
    }

    fn log(&self) {
        tracing::debug!(settings = ?self, "macOS notification settings");
        if let Some(problem) = self.problem() {
            tracing::warn!(problem, "macOS may not show or sound NODIrust alarms");
        }
        if let Some(time_sensitive) = self
            .time_sensitive
            .filter(|setting| *setting != UNNotificationSetting::Enabled)
        {
            // Ad-hoc signed builds cannot carry the restricted time-sensitive
            // entitlement, so this is the expected state for release DMGs.
            tracing::info!(
                setting = ?time_sensitive,
                "macOS will not treat NODIrust alarms as Time Sensitive; Focus can still hold them"
            );
        }
    }
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements, and this type has no
    // ivars or Drop implementation.
    #[unsafe(super(NSObject))]
    #[name = "NODIrustNotificationDelegate"]
    struct NotificationDelegate;

    unsafe impl NSObjectProtocol for NotificationDelegate {}

    unsafe impl UNUserNotificationCenterDelegate for NotificationDelegate {
        // Without this, macOS drops banners and sounds while NODIrust is the active app.
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn will_present(
            &self,
            _center: &UNUserNotificationCenter,
            _notification: &UNNotification,
            completion_handler: &block2::DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
        ) {
            completion_handler.call((UNNotificationPresentationOptions::Banner
                | UNNotificationPresentationOptions::List
                | UNNotificationPresentationOptions::Sound,));
        }

        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn did_receive_response(
            &self,
            _center: &UNUserNotificationCenter,
            response: &UNNotificationResponse,
            completion_handler: &block2::DynBlock<dyn Fn()>,
        ) {
            // SAFETY: UserNotifications exports this constant for the process lifetime.
            let default_action = unsafe { UNNotificationDefaultActionIdentifier };
            if &*response.actionIdentifier() == default_action {
                open_ui();
            }
            completion_handler.call(());
        }
    }
);

impl NotificationDelegate {
    fn new() -> Retained<Self> {
        let this = Self::alloc().set_ivars(());
        // SAFETY: `init` is NSObject's designated initializer.
        unsafe { msg_send![super(this), init] }
    }
}

#[cfg(test)]
mod tests {
    use super::DeliverySettings;
    use objc2_user_notifications::{UNAlertStyle, UNAuthorizationStatus, UNNotificationSetting};

    fn allowed() -> DeliverySettings {
        DeliverySettings {
            authorization: UNAuthorizationStatus::Authorized,
            alerts: UNNotificationSetting::Enabled,
            alert_style: UNAlertStyle::Banner,
            sound: UNNotificationSetting::Enabled,
            notification_center: UNNotificationSetting::Enabled,
            time_sensitive: Some(UNNotificationSetting::Disabled),
        }
    }

    #[test]
    fn accepts_banner_with_sound_even_without_time_sensitive() {
        assert_eq!(allowed().problem(), None);
    }

    #[test]
    fn reports_denied_permission_first() {
        let settings = DeliverySettings {
            authorization: UNAuthorizationStatus::Denied,
            sound: UNNotificationSetting::Disabled,
            ..allowed()
        };
        assert!(
            settings
                .problem()
                .is_some_and(|text| text.contains("Allow Notifications"))
        );
    }

    #[test]
    fn reports_missing_banner() {
        let settings = DeliverySettings {
            alert_style: UNAlertStyle::None,
            ..allowed()
        };
        assert!(
            settings
                .problem()
                .is_some_and(|text| text.contains("Banners are off"))
        );
    }

    #[test]
    fn reports_disabled_sound() {
        let settings = DeliverySettings {
            sound: UNNotificationSetting::Disabled,
            ..allowed()
        };
        assert!(
            settings
                .problem()
                .is_some_and(|text| text.contains("silent"))
        );
    }
}
