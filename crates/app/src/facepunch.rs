pub struct FacepunchClient {
    client: reqwest::Client,
    steam_token: String,
}

impl FacepunchClient {
    pub fn new(steam_token: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            steam_token: steam_token.into(),
        }
    }

    /// Registers the FCM push token with Facepunch so this device receives notifications
    ///
    /// # Errors
    /// Returns an error if the request fails or serialization fails.
    pub async fn register_push(&self, expo_token: &str) -> Result<(), reqwest::Error> {
        let url = "https://companion-rust.facepunch.com/api/push/register";

        // Facepunch requires AuthToken, PushToken, DeviceId, and PushKind
        let payload = serde_json::json!({
            "AuthToken": self.steam_token,
            "PushToken": expo_token,
            "DeviceId": "nodirust_desktop",
            "PushKind": 3
        });

        self.client
            .post(url)
            .json(&payload)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}
