//! Events mapping protocol triggers to UI and notifications.

pub enum DaemonEvent {
    ConnectionStatusChanged(crate::ipc::ConnectionStatus),
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
        status: crate::ipc::ConnectionStatus,
    },
    ServerNameDiscovered {
        ip: String,
        port: u16,
        name: String,
    },
}
