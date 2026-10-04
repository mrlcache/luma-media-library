use std::{
    collections::BTreeMap,
    net::UdpSocket,
    time::{Duration, Instant},
};

#[tauri::command]
pub async fn discover_luma_computers() -> Result<Vec<serde_json::Value>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let socket=UdpSocket::bind("0.0.0.0:0").map_err(|e|e.to_string())?;
        socket.set_broadcast(true).map_err(|e|e.to_string())?;
        socket.set_read_timeout(Some(Duration::from_millis(200))).map_err(|e|e.to_string())?;
        socket.send_to(b"LUMA_DISCOVER_V1","255.255.255.255:47631").map_err(|e|e.to_string())?;
        let until=Instant::now()+Duration::from_secs(2);
        let mut found=BTreeMap::new(); let mut packet=[0u8;512];
        while Instant::now()<until {
            if let Ok((count,peer))=socket.recv_from(&mut packet) {
                let Ok(value)=serde_json::from_slice::<serde_json::Value>(&packet[..count]) else {continue;};
                if value.get("service").and_then(|s|s.as_str())!=Some("luma") || !peer.is_ipv4() {continue;}
                let url=format!("http://{}:47631",peer.ip());
                found.insert(url.clone(),serde_json::json!({"url":url,"name":value.get("name").and_then(|s|s.as_str()).unwrap_or("Luma Desktop")}));
            }
        }
        Ok(found.into_values().collect())
    }).await.map_err(|e|e.to_string())?
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(6))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())
}
fn address(raw: &str) -> Result<String, String> {
    let url = reqwest::Url::parse(raw.trim()).map_err(|_| "Invalid computer address")?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Invalid computer address".into());
    }
    Ok(raw.trim().trim_end_matches('/').into())
}
#[tauri::command]
pub async fn request_mobile_pair(url: String) -> Result<serde_json::Value, String> {
    let response = client()?
        .post(format!("{}/api/v1/pair/request", address(&url)?))
        .json(&serde_json::json!({"name":"Luma Mobile"}))
        .send()
        .await
        .map_err(|e| format!("Could not reach your computer: {e}"))?;
    response
        .error_for_status()
        .map_err(|e| format!("Could not start pairing: {e}"))?
        .json()
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn check_mobile_pair(
    app: tauri::AppHandle,
    url: String,
    id: String,
) -> Result<bool, String> {
    if id.len() != 64 || !id.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("Invalid pair request".into());
    }
    let url = address(&url)?;
    let response = client()?
        .get(format!("{url}/api/v1/pair/status?id={id}"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err("Pair request expired or declined. Try again.".into());
    }
    let value: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    if value.get("paired").and_then(|s| s.as_bool()) == Some(true) {
        let token = value
            .get("token")
            .and_then(|s| s.as_str())
            .ok_or("Missing pair key")?;
        super::set_mobile_connection(app, url, token.into())?;
        return Ok(true);
    }
    Ok(false)
}
