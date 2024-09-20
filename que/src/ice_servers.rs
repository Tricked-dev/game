use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::UserCreateError;

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct IceServers {
    urls: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    credential: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IceServersResponse {
    #[serde(rename = "iceServers")]
    ice_servers: IceServers,
}

#[derive(Debug, Clone)]
pub enum IceServerProvider {
    Google(GoogleIceServerProvider),
    Cloudflare(CloudflareIceServerProvider),
}

impl IceServerProvider {
    pub async fn get_ice_servers(&self) -> Result<IceServers, UserCreateError> {
        match self {
            IceServerProvider::Google(provider) => provider.get_ice_servers().await,
            IceServerProvider::Cloudflare(provider) => provider.get_ice_servers().await,
        }
    }
}

pub type IceServerData = Arc<IceServerProvider>;

#[derive(Debug, Default, Clone)]
pub struct CloudflareIceServerProvider {
    turn_token_id: String,
    api_token: String,
}

impl CloudflareIceServerProvider {
    pub fn new(turn_token_id: String, api_token: String) -> Self {
        Self {
            turn_token_id,
            api_token,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct GoogleIceServerProvider;

impl GoogleIceServerProvider {
    async fn get_ice_servers(&self) -> Result<IceServers, UserCreateError> {
        Ok(IceServers {
            urls: vec![
                "stun:stun.l.google.com:19302".to_owned(),
                "stun:stun1.l.google.com:19302".to_owned(),
                "stun:stun2.l.google.com:19302".to_owned(),
            ],
            ..Default::default()
        })
    }
}

impl CloudflareIceServerProvider {
    async fn get_ice_servers(&self) -> Result<IceServers, UserCreateError> {
        let url = format!(
            "https://rtc.live.cloudflare.com/v1/turn/keys/{}/credentials/generate",
            self.turn_token_id
        );
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            reqwest::header::HeaderValue::from_str(&format!("Bearer {}", self.api_token))
                .unwrap(),
        );
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/json"),
        );
        let client = reqwest::Client::new();
        let response = client
            .post(url)
            .headers(headers)
            .body(r#"{"ttl":900}"#)
            .send()
            .await;

        let response = response.map_err(|e| UserCreateError::Internal(e.to_string()))?;

        let response = response.json::<IceServersResponse>().await?;
        Ok(response.ice_servers)
    }
}
