use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HomelabApp {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub local_url: String,
    pub remote_url: Option<String>,
    pub local_host: Option<String>,
    pub local_port: Option<u16>,
    pub remote_host: Option<String>,
    pub remote_port: Option<u16>,
    pub connection_mode: String,
    pub credential_id: Option<String>,
    pub order: usize,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Credential {
    pub id: String,
    pub name: String,
    pub username: Option<String>,
    pub secret_reference: String,
    pub notes: Option<String>,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NetworkProfile {
    pub id: String,
    pub name: Option<String>,
    pub interface_type: String,
    pub ssid: Option<String>,
    pub gateway_ip: Option<String>,
    pub gateway_mac: Option<String>,
    pub subnet: Option<String>,
    pub trusted_local: bool,
    #[serde(default)]
    pub local_override: Option<bool>,
    pub last_seen: String,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TrustedCertificate {
    pub app_id: String,
    pub host: String,
    pub port: u16,
    pub sha256: String,
    pub expires_at: i64,
}
#[derive(Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub apps: Vec<HomelabApp>,
    pub credentials: Vec<Credential>,
    pub networks: Vec<NetworkProfile>,
    #[serde(default)]
    pub trusted_certificates: Vec<TrustedCertificate>,
}
#[derive(Clone, Serialize, Debug)]
pub struct Resolution {
    pub available: Option<bool>,
    pub url: Option<String>,
    pub status: String,
    pub reason: String,
}
pub fn seed() -> Config {
    let mut config = Config::default();
    for (i, (name, icon, url)) in [
        ("Home Assistant", "house", "http://192.168.0.20:8123"),
        ("Proxmox", "server", "https://192.168.0.10:8006"),
        ("n8n", "workflow", "http://192.168.0.30:5678"),
        ("AI Services", "sparkles", "http://192.168.0.51"),
    ]
    .iter()
    .enumerate()
    {
        config.apps.push(HomelabApp {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            icon: Some(icon.to_string()),
            local_url: url.to_string(),
            remote_url: None,
            local_host: None,
            local_port: None,
            remote_host: None,
            remote_port: None,
            connection_mode: "auto".into(),
            credential_id: None,
            order: i,
        });
    }
    config
}
