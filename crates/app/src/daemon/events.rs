//! Events mapping protocol triggers to UI and notifications.

pub enum DaemonEvent {
    ConnectionStatusChanged(bool),
    PushNotificationReceived {
        title: String,
        body: String,
    },
    ServerChatReceived {
        server_ip: String,
        sender: String,
        message: String,
    },
    PairingRequest(crate::config::store::ServerConfig),
    EntityPairingRequest(crate::config::store::DeviceConfig),
    ServerConnectionStatusChanged {
        server_ip_port: String,
        connected: bool,
    },
}
