use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct ExpoPushRequest<'a> {
    #[serde(rename = "appId")]
    app_id: &'a str,
    #[serde(rename = "deviceId")]
    device_id: &'a str,
    #[serde(rename = "deviceToken")]
    device_token: &'a str,
    #[serde(rename = "type")]
    device_type: &'a str,
    #[serde(rename = "experienceId")]
    experience_id: &'a str,
}

#[derive(Deserialize)]
struct ExpoPushResponse {
    data: ExpoPushData,
}

#[derive(Deserialize)]
struct ExpoPushData {
    #[serde(rename = "expoPushToken")]
    expo_push_token: String,
}

/// Exchanges a raw Android FCM token for an Expo Push Token.
pub async fn get_expo_push_token(
    client: &reqwest::Client,
    fcm_token: &str,
    device_id: &str,
) -> Result<String, reqwest::Error> {
    let url = "https://exp.host/--/api/v2/push/getExpoPushToken";
    let req = ExpoPushRequest {
        app_id: "com.facepunch.rust.companion",
        device_id,
        device_token: fcm_token,
        device_type: "fcm",
        experience_id: "@facepunch/RustCompanion",
    };

    let res: ExpoPushResponse = client
        .post(url)
        .json(&req)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(res.data.expo_push_token)
}
