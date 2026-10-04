use std::{
    collections::{BTreeMap, HashSet, VecDeque},
    net::UdpSocket,
    sync::Mutex,
    time::{Duration, Instant},
};

const MAX_CANCELLED_PAIR_REQUESTS: usize = 256;

#[derive(Default)]
pub struct PairingState {
    lifecycle: Mutex<PairingLifecycle>,
}

#[derive(Default)]
struct PairingLifecycle {
    cancelled_ids: HashSet<String>,
    cancelled_order: VecDeque<String>,
    committed_id: Option<String>,
}

impl PairingLifecycle {
    fn cancel(&mut self, id: &str) {
        if self.cancelled_ids.insert(id.to_owned()) {
            self.cancelled_order.push_back(id.to_owned());
        }
        while self.cancelled_order.len() > MAX_CANCELLED_PAIR_REQUESTS {
            if let Some(expired) = self.cancelled_order.pop_front() {
                self.cancelled_ids.remove(&expired);
            }
        }
    }
}

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
                let Some(port)=value.get("port").and_then(|p|p.as_u64()).filter(|p| (1024..=65535).contains(p)) else {continue;};
                let url=format!("http://{}:{port}",peer.ip());
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
    state: tauri::State<'_, PairingState>,
) -> Result<bool, String> {
    if id.len() != 64 || !id.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("Invalid pair request".into());
    }
    if state
        .lifecycle
        .lock()
        .map_err(|_| "Pairing state is unavailable".to_owned())?
        .cancelled_ids
        .contains(&id)
    {
        return Ok(false);
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
        let mut lifecycle = state
            .lifecycle
            .lock()
            .map_err(|_| "Pairing state is unavailable".to_owned())?;
        if lifecycle.cancelled_ids.contains(&id) {
            return Ok(false);
        }
        super::set_mobile_connection(app, url, token.into())?;
        lifecycle.committed_id = Some(id);
        return Ok(true);
    }
    Ok(false)
}

#[tauri::command]
pub async fn cancel_mobile_pair(
    app: tauri::AppHandle,
    url: String,
    id: String,
    state: tauri::State<'_, PairingState>,
) -> Result<(), String> {
    if id.len() != 64 || !id.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("Invalid pair request".into());
    }

    let local_result = {
        let mut lifecycle = state
            .lifecycle
            .lock()
            .map_err(|_| "Pairing state is unavailable".to_owned())?;
        lifecycle.cancel(&id);
        if lifecycle.committed_id.as_deref() == Some(id.as_str()) {
            let result = super::clear_mobile_connection(app.clone());
            if result.is_ok() {
                lifecycle.committed_id = None;
            }
            result
        } else {
            Ok(())
        }
    };

    // Mark the attempt locally before awaiting the network so an in-flight
    // status response cannot commit after cancellation.
    let remote_result = async {
        let url = address(&url)?;
        client()?
            .post(format!("{url}/api/v1/pair/cancel?id={id}"))
            .send()
            .await
            .map_err(|e| format!("Could not cancel pairing request: {e}"))?
            .error_for_status()
            .map(|_| ())
            .map_err(|e| format!("Could not cancel pairing request: {e}"))
    }
    .await;

    local_result?;
    remote_result?;
    Ok(())
}
