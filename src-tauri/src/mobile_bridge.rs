//! Opt-in authenticated HTTP bridge for the native mobile client.
use serde::Serialize;
use std::{
    collections::HashMap,
    io::{self, Read, Seek, Write},
    net::{TcpListener, TcpStream, UdpSocket},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::Manager;

const DEFAULT_PORT: u16 = 47631;
const DISCOVERY_PORT: u16 = 47631;
const MIN_BRIDGE_PORT: u16 = 1024;
const MAX_BODY: usize = 8 * 1024 * 1024;
const ADMIN_ORIGIN: &str = "http://127.0.0.1:8940";
const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;

#[derive(Clone)]
pub struct BridgeState {
    inner: Arc<Mutex<BridgeConfig>>,
    token_path: PathBuf,
    devices: Arc<Mutex<HashMap<String, CastDevice>>>,
    release_service: Arc<Mutex<ServiceChild>>,
    pairing: Arc<Mutex<HashMap<String, PairRequest>>>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairRequest {
    id: String,
    name: String,
    code: String,
    expires: u64,
    approved: bool,
}
struct ServiceChild(Option<std::process::Child>);
impl Drop for ServiceChild {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
#[derive(Clone)]
struct CastDevice {
    name: String,
    control_url: String,
}
struct BridgeConfig {
    enabled: bool,
    listening: bool,
    discovery_started: bool,
    port: u16,
    listener_stop: Option<Arc<AtomicBool>>,
    token: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeInfo {
    enabled: bool,
    port: u16,
    base_url: Option<String>,
    token: Option<String>,
}

impl BridgeState {
    pub fn new(data: PathBuf) -> Self {
        let token_path = data.join("mobile-bridge-token");
        let token = std::fs::read_to_string(&token_path)
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| s.len() >= 32)
            .unwrap_or_else(new_token);
        let _ = std::fs::write(&token_path, &token);
        let port = std::fs::read_to_string(token_path.with_extension("port"))
            .ok()
            .and_then(|value| value.trim().parse::<u16>().ok())
            .filter(|port| *port >= MIN_BRIDGE_PORT)
            .unwrap_or(DEFAULT_PORT);
        Self {
            inner: Arc::new(Mutex::new(BridgeConfig {
                enabled: false,
                listening: false,
                discovery_started: false,
                port,
                listener_stop: None,
                token,
            })),
            token_path,
            devices: Arc::new(Mutex::new(HashMap::new())),
            release_service: Arc::new(Mutex::new(ServiceChild(None))),
            pairing: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    pub fn set(&self, app: tauri::AppHandle, enabled: bool) -> Result<BridgeInfo, String> {
        let mut cfg = self
            .inner
            .lock()
            .map_err(|_| "Mobile bridge state is unavailable.".to_owned())?;
        if enabled && !cfg.listening {
            let listener = bind_bridge_listener(cfg.port)?;
            let discovery_socket = if cfg.discovery_started {
                None
            } else {
                Some(
                    UdpSocket::bind(("0.0.0.0", DISCOVERY_PORT)).map_err(|e| {
                        format!("Could not start LAN discovery on port {DISCOVERY_PORT}: {e}")
                    })?,
                )
            };
            let stop = spawn_bridge_listener(listener, app, self.inner.clone())?;
            if let Some(socket) = discovery_socket {
                if let Err(error) = spawn_discovery_listener(socket, self.inner.clone()) {
                    stop.store(true, Ordering::Release);
                    return Err(error);
                }
                cfg.discovery_started = true;
            }
            cfg.listener_stop = Some(stop);
            cfg.listening = true;
        }
        std::fs::write(
            self.token_path.with_extension("enabled"),
            if enabled { "1" } else { "0" },
        )
        .map_err(|e| format!("Could not save mobile bridge preference: {e}"))?;
        cfg.enabled = enabled;
        Ok(info(&cfg))
    }
    pub fn set_port(
        &self,
        app: tauri::AppHandle,
        requested_port: u32,
    ) -> Result<BridgeInfo, String> {
        if !(MIN_BRIDGE_PORT as u32..=u16::MAX as u32).contains(&requested_port) {
            return Err(format!(
                "Choose a mobile connection port between {MIN_BRIDGE_PORT} and {}.",
                u16::MAX
            ));
        }
        let port = requested_port as u16;
        let mut cfg = self
            .inner
            .lock()
            .map_err(|_| "Mobile bridge state is unavailable.".to_owned())?;
        if port == cfg.port {
            return Ok(info(&cfg));
        }

        // Keep the current listener alive until the replacement has bound and
        // started. A failed change therefore leaves existing connections up.
        let replacement = if cfg.listening {
            let listener = bind_bridge_listener(port)?;
            Some(spawn_bridge_listener(listener, app, self.inner.clone())?)
        } else {
            None
        };
        if let Err(error) =
            std::fs::write(self.token_path.with_extension("port"), port.to_string())
        {
            if let Some(stop) = &replacement {
                stop.store(true, Ordering::Release);
            }
            return Err(format!("Could not save mobile connection port: {error}"));
        }
        cfg.port = port;
        if let Some(replacement) = replacement {
            if let Some(previous) = cfg.listener_stop.replace(replacement) {
                previous.store(true, Ordering::Release);
            }
        }
        Ok(info(&cfg))
    }
    pub fn restore(&self, app: tauri::AppHandle) -> Result<(), String> {
        if std::fs::read_to_string(self.token_path.with_extension("enabled"))
            .ok()
            .as_deref()
            == Some("1")
        {
            self.set(app, true)?;
        }
        Ok(())
    }
    fn ensure_release_service(&self, app: &tauri::AppHandle) -> Result<(), String> {
        let mut child = self
            .release_service
            .lock()
            .map_err(|_| "Release service state is unavailable")?;
        if let Some(process) = &mut child.0 {
            if process.try_wait().map_err(|e| e.to_string())?.is_none() {
                return Ok(());
            }
        }
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/release-service");
        let root = if cfg!(debug_assertions) {
            source
        } else {
            app.path()
                .resource_dir()
                .map_err(|e| e.to_string())?
                .join("release-service")
        };
        let mut command = std::process::Command::new(root.join("node.exe"));
        command
            .arg(root.join("release-service.mjs"))
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .env(
                "LUMA_APP_DATA",
                app.path().app_data_dir().map_err(|e| e.to_string())?,
            );
        let config = if cfg!(debug_assertions) {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../.artifacts/tools/prowlarr/data/config.xml")
        } else {
            app.path()
                .app_data_dir()
                .map_err(|e| e.to_string())?
                .join("prowlarr/config.xml")
        };
        command.env("LUMA_PROWLARR_CONFIG", config);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        child.0 = Some(
            command
                .spawn()
                .map_err(|e| format!("Could not start bundled release search: {e}"))?,
        );
        for _ in 0..40 {
            if TcpStream::connect_timeout(
                &"127.0.0.1:8943".parse().unwrap(),
                std::time::Duration::from_millis(50),
            )
            .is_ok()
            {
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        Err("Release search did not start. Restart Luma and try again.".into())
    }
    pub fn info(&self) -> BridgeInfo {
        self.inner.lock().map(|c| info(&c)).unwrap_or(BridgeInfo {
            enabled: false,
            port: DEFAULT_PORT,
            base_url: None,
            token: None,
        })
    }
}
#[tauri::command]
pub fn get_mobile_pair_requests(state: tauri::State<'_, BridgeState>) -> Vec<PairRequest> {
    let Ok(mut requests) = state.pairing.lock() else {
        return vec![];
    };
    requests.retain(|_, request| request.expires > now_seconds());
    requests
        .values()
        .filter(|request| !request.approved)
        .cloned()
        .collect()
}
#[tauri::command]
pub fn approve_mobile_pair(
    id: String,
    allow: bool,
    state: tauri::State<'_, BridgeState>,
) -> Result<(), String> {
    let mut requests = state.pairing.lock().map_err(|_| "Pairing is unavailable")?;
    let request = requests
        .get_mut(&id)
        .filter(|r| r.expires > now_seconds())
        .ok_or("Pair request expired")?;
    if allow {
        request.approved = true;
    } else {
        requests.remove(&id);
    }
    Ok(())
}
fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
#[tauri::command]
pub fn get_mobile_bridge_info(state: tauri::State<'_, BridgeState>) -> BridgeInfo {
    state.info()
}
#[tauri::command]
pub fn set_mobile_bridge_enabled(
    enabled: bool,
    state: tauri::State<'_, BridgeState>,
    app: tauri::AppHandle,
) -> Result<BridgeInfo, String> {
    state.set(app, enabled)
}
#[tauri::command]
pub fn set_mobile_bridge_port(
    port: u32,
    state: tauri::State<'_, BridgeState>,
    app: tauri::AppHandle,
) -> Result<BridgeInfo, String> {
    state.set_port(app, port)
}
#[tauri::command]
pub fn enable_mobile_bridge(
    enabled: bool,
    state: tauri::State<'_, BridgeState>,
    app: tauri::AppHandle,
) -> Result<BridgeInfo, String> {
    state.set(app, enabled)
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CastDeviceInfo {
    id: String,
    name: String,
}
#[tauri::command]
pub fn discover_cast_devices(
    state: tauri::State<'_, BridgeState>,
) -> Result<Vec<CastDeviceInfo>, String> {
    discover(state.inner())
}
#[tauri::command]
pub fn cast_media(
    device_id: String,
    media_id: i64,
    state: tauri::State<'_, BridgeState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    cast(&state, &app, &device_id, media_id)
}
fn discover(state: &BridgeState) -> Result<Vec<CastDeviceInfo>, String> {
    use std::net::UdpSocket;
    let socket =
        UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("Could not open LAN discovery: {e}"))?;
    socket
        .set_read_timeout(Some(std::time::Duration::from_millis(180)))
        .ok();
    let request="M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 1\r\nST: urn:schemas-upnp-org:device:MediaRenderer:1\r\n\r\n";
    socket
        .send_to(request.as_bytes(), "239.255.255.250:1900")
        .map_err(|e| format!("Could not send UPnP discovery: {e}"))?;
    let until = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let mut locations = std::collections::HashSet::new();
    let mut buf = [0u8; 8192];
    while std::time::Instant::now() < until {
        match socket.recv_from(&mut buf) {
            Ok((n, _)) => {
                let text = String::from_utf8_lossy(&buf[..n]);
                if let Some(url) = text.lines().find_map(|l| {
                    l.split_once(':')
                        .filter(|(k, _)| k.eq_ignore_ascii_case("location"))
                        .map(|(_, v)| v.trim().to_owned())
                }) {
                    locations.insert(url);
                }
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(_) => break,
        }
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?;
    let mut found = HashMap::new();
    for location in locations.into_iter().take(32) {
        if !private_http_url(&location) {
            continue;
        }
        let Ok(response) = client.get(&location).send() else {
            continue;
        };
        if !response.status().is_success() {
            continue;
        }
        let mut xml = String::new();
        if response.take(262145).read_to_string(&mut xml).is_err() || xml.len() > 262144 {
            continue;
        }
        let Some(name) = xml_tag(&xml, "friendlyName") else {
            continue;
        };
        if !xml.contains("MediaRenderer") || !xml.contains("AVTransport") {
            continue;
        }
        let Some(control) = xml
            .split("<service>")
            .skip(1)
            .filter_map(|part| part.split_once("</service>").map(|p| p.0))
            .find(|service| {
                xml_tag(service, "serviceType").is_some_and(|s| s.contains(":AVTransport:"))
            })
            .and_then(|service| xml_tag(service, "controlURL"))
        else {
            continue;
        };
        let Ok(base) = reqwest::Url::parse(&location) else {
            continue;
        };
        let Ok(control_url) = base.join(&control) else {
            continue;
        };
        let control_url = control_url.to_string();
        if !private_http_url(&control_url) {
            continue;
        }
        let token = state
            .inner
            .lock()
            .map(|c| c.token.clone())
            .unwrap_or_default();
        let id = sign_device(&token, &location);
        found.insert(
            id.clone(),
            CastDevice {
                name: name.clone(),
                control_url,
            },
        );
    }
    let mut cache = state
        .devices
        .lock()
        .map_err(|_| "Cast device list is unavailable.".to_owned())?;
    *cache = found.clone();
    Ok(found
        .into_iter()
        .map(|(id, d)| CastDeviceInfo { id, name: d.name })
        .collect())
}
fn xml_tag(xml: &str, tag: &str) -> Option<String> {
    let lower = xml.to_ascii_lowercase();
    let open = format!("<{tag}");
    let start = lower.find(&open)?;
    let content = lower[start..].find('>')? + start + 1;
    let close = lower[content..].find(&format!("</{tag}>"))? + content;
    Some(xml[content..close].replace("&amp;", "&").trim().to_owned())
}
fn private_http_url(raw: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(raw) else {
        return false;
    };
    if url.scheme() != "http" || !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    let addrs = match host.parse::<std::net::IpAddr>() {
        Ok(ip) => vec![ip],
        Err(_) => return false,
    };
    !addrs.is_empty()
        && addrs.iter().all(|ip| match ip {
            std::net::IpAddr::V4(v) => v.is_private() || v.is_loopback() || v.is_link_local(),
            std::net::IpAddr::V6(v) => {
                v.is_loopback() || v.is_unique_local() || v.is_unicast_link_local()
            }
        })
}
fn sign_device(key: &str, location: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let mut mac = Hmac::<Sha256>::new_from_slice(key.as_bytes()).unwrap();
    mac.update(location.as_bytes());
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn bind_bridge_listener(port: u16) -> Result<TcpListener, String> {
    let listener = TcpListener::bind(("0.0.0.0", port))
        .map_err(|e| format!("Could not bind the mobile bridge to port {port}: {e}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("Could not prepare the mobile bridge on port {port}: {e}"))?;
    Ok(listener)
}
fn spawn_bridge_listener(
    listener: TcpListener,
    app: tauri::AppHandle,
    state: Arc<Mutex<BridgeConfig>>,
) -> Result<Arc<AtomicBool>, String> {
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    std::thread::Builder::new()
        .name("luma-mobile-bridge".into())
        .spawn(move || serve(listener, app, state, worker_stop))
        .map_err(|e| format!("Could not start mobile bridge listener: {e}"))?;
    Ok(stop)
}
fn spawn_discovery_listener(
    socket: UdpSocket,
    state: Arc<Mutex<BridgeConfig>>,
) -> Result<(), String> {
    std::thread::Builder::new()
        .name("luma-lan-discovery".into())
        .spawn(move || {
            let mut packet = [0u8; 512];
            while let Ok((count, peer)) = socket.recv_from(&mut packet) {
                if packet[..count] != *b"LUMA_DISCOVER_V1" {
                    continue;
                }
                let Ok(config) = state.lock() else {
                    continue;
                };
                if !config.enabled {
                    continue;
                }
                let port = config.port;
                drop(config);
                let message = serde_json::json!({
                    "service": "luma",
                    "name": std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Luma Desktop".into()),
                    "port": port,
                })
                .to_string();
                let _ = socket.send_to(message.as_bytes(), peer);
            }
        })
        .map_err(|e| format!("Could not start LAN discovery listener: {e}"))?;
    Ok(())
}
fn cast(
    state: &BridgeState,
    app: &tauri::AppHandle,
    id: &str,
    media_id: i64,
) -> Result<(), String> {
    state.set(app.clone(), true)?;
    let device = state
        .devices
        .lock()
        .map_err(|_| "Cast device list is unavailable.".to_owned())?
        .get(id)
        .cloned()
        .ok_or_else(|| {
            "The selected TV is no longer available. Discover devices again.".to_owned()
        })?;
    if !private_http_url(&device.control_url) {
        return Err("The selected TV is outside the local network.".into());
    }
    let library = app.state::<media_core::LibraryState>();
    let path = media_core::LibraryStore::open(&library.db_path)?
        .resolve_media_path(media_id)?
        .ok_or_else(|| "This media file is no longer in the library.".to_owned())?;
    let exp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        + 21600;
    let (token, port) = {
        let config = state
            .inner
            .lock()
            .map_err(|_| "Mobile bridge state is unavailable.".to_owned())?;
        (config.token.clone(), config.port)
    };
    let host = lan_ip();
    let media_url = format!(
        "http://{host}:{port}/api/v1/media/{media_id}?expires={exp}&signature={}",
        sign(&token, media_id, exp)
    );
    let title = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Luma media")
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let _ = title;
    let media_url = media_url.replace('&', "&amp;");
    let body=format!("<?xml version=\"1.0\"?><s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\"><s:Body><u:SetAVTransportURI xmlns:u=\"urn:schemas-upnp-org:service:AVTransport:1\"><InstanceID>0</InstanceID><CurrentURI>{media_url}</CurrentURI><CurrentURIMetaData></CurrentURIMetaData></u:SetAVTransportURI></s:Body></s:Envelope>");
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .post(&device.control_url)
        .header("Content-Type", "text/xml; charset=\"utf-8\"")
        .header(
            "SOAPACTION",
            "\"urn:schemas-upnp-org:service:AVTransport:1#SetAVTransportURI\"",
        )
        .body(body)
        .send()
        .map_err(|e| format!("Could not connect to the TV: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "TV rejected the media URL (HTTP {}).",
            response.status()
        ));
    }
    let play="<?xml version=\"1.0\"?><s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\"><s:Body><u:Play xmlns:u=\"urn:schemas-upnp-org:service:AVTransport:1\"><InstanceID>0</InstanceID><Speed>1</Speed></u:Play></s:Body></s:Envelope>";
    let response = client
        .post(&device.control_url)
        .header("Content-Type", "text/xml; charset=\"utf-8\"")
        .header(
            "SOAPACTION",
            "\"urn:schemas-upnp-org:service:AVTransport:1#Play\"",
        )
        .body(play)
        .send()
        .map_err(|e| format!("Could not start TV playback: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "TV could not start playback (HTTP {}).",
            response.status()
        ));
    }
    Ok(())
}
fn info(c: &BridgeConfig) -> BridgeInfo {
    BridgeInfo {
        enabled: c.enabled,
        port: c.port,
        base_url: c.enabled.then(|| format!("http://{}:{}", lan_ip(), c.port)),
        token: c.enabled.then(|| c.token.clone()),
    }
}
fn lan_ip() -> String {
    UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| {
            s.connect("192.0.2.1:9")?;
            s.local_addr()
        })
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".into())
}
fn new_token() -> String {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b).expect("The operating system must provide secure pairing keys");
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn serve(
    listener: TcpListener,
    app: tauri::AppHandle,
    state: Arc<Mutex<BridgeConfig>>,
    stop: Arc<AtomicBool>,
) {
    use std::sync::atomic::AtomicUsize;
    let requests = Arc::new(AtomicUsize::new(0));
    while !stop.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                if requests.fetch_add(1, Ordering::Relaxed) >= 16 {
                    requests.fetch_sub(1, Ordering::Relaxed);
                    continue;
                }
                let a = app.clone();
                let s = state.clone();
                let counter = requests.clone();
                let spawned = std::thread::Builder::new()
                    .name("luma-mobile-request".into())
                    .spawn(move || {
                        handle(&mut stream, &a, &s);
                        counter.fetch_sub(1, Ordering::Relaxed);
                    });
                if spawned.is_err() {
                    requests.fetch_sub(1, Ordering::Relaxed);
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(_) => break,
        }
    }
}
fn handle(stream: &mut TcpStream, app: &tauri::AppHandle, state: &Arc<Mutex<BridgeConfig>>) {
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(15)));
    let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(30)));
    let mut data = Vec::new();
    let mut buf = [0u8; 8192];
    let split;
    loop {
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => return,
            Ok(n) => {
                data.extend_from_slice(&buf[..n]);
                if let Some(p) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                    split = p + 4;
                    break;
                }
                if data.len() > 65536 {
                    return;
                }
            }
        }
    }
    let head = String::from_utf8_lossy(&data[..split]).into_owned();
    let mut lines = head.lines();
    let first = lines.next().unwrap_or("").to_owned();
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("").to_owned();
    let target = parts.next().unwrap_or("/").to_owned();
    let mut length = 0usize;
    let mut authorized = false;
    let enabled = state.lock().map(|c| c.enabled).unwrap_or(false);
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            if k.eq_ignore_ascii_case("content-length") {
                length = v.trim().parse().unwrap_or(MAX_BODY + 1);
            }
            if k.eq_ignore_ascii_case("authorization") {
                let token = state.lock().map(|c| c.token.clone()).unwrap_or_default();
                authorized = v.trim() == format!("Bearer {token}");
            }
        }
    }
    if length > MAX_BODY {
        reply(
            stream,
            413,
            "{\"error\":\"Request too large\"}",
            "application/json",
        );
        return;
    }
    while data.len() < split + length {
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => return,
            Ok(n) => data.extend_from_slice(&buf[..n]),
        }
    }
    let path = target.split('?').next().unwrap_or("/");
    if enabled && method == "POST" && path == "/api/v1/pair/request" {
        let value: serde_json::Value =
            serde_json::from_slice(&data[split..split + length]).unwrap_or_default();
        let name = value
            .get("name")
            .and_then(|s| s.as_str())
            .unwrap_or("Phone")
            .chars()
            .take(48)
            .collect::<String>();
        let bridge = app.state::<BridgeState>();
        let Ok(mut requests) = bridge.pairing.lock() else {
            return;
        };
        requests.retain(|_, r| r.expires > now_seconds());
        if requests.len() >= 8 {
            reply(
                stream,
                429,
                "{\"error\":\"Too many pairing requests. Try again shortly.\"}",
                "application/json",
            );
            return;
        }
        let id = new_token();
        let random = new_token();
        let code = format!(
            "{:06}",
            u32::from_str_radix(&random[..8], 16).unwrap_or(0) % 1_000_000
        );
        let request = PairRequest {
            id: id.clone(),
            name,
            code,
            expires: now_seconds() + 120,
            approved: false,
        };
        let body = serde_json::to_string(&request).unwrap();
        requests.insert(id, request);
        reply(stream, 200, &body, "application/json");
        return;
    }
    if enabled && method == "POST" && path == "/api/v1/pair/cancel" {
        let id = target
            .split_once('?')
            .and_then(|(_, query)| query.split('&').find_map(|field| field.strip_prefix("id=")))
            .unwrap_or("");
        if id.len() != 64 || !id.bytes().all(|c| c.is_ascii_hexdigit()) {
            reply(
                stream,
                400,
                "{\"error\":\"Invalid pair request\"}",
                "application/json",
            );
            return;
        }
        let bridge = app.state::<BridgeState>();
        let Ok(mut requests) = bridge.pairing.lock() else {
            reply(
                stream,
                503,
                "{\"error\":\"Pairing is unavailable\"}",
                "application/json",
            );
            return;
        };
        requests.remove(id);
        reply(stream, 200, "{\"cancelled\":true}", "application/json");
        return;
    }
    if enabled && method == "GET" && path == "/api/v1/pair/status" {
        let id = target.split_once("?id=").map(|s| s.1).unwrap_or("");
        let bridge = app.state::<BridgeState>();
        let Ok(mut requests) = bridge.pairing.lock() else {
            return;
        };
        let Some(request) = requests.get(id).filter(|r| r.expires > now_seconds()) else {
            reply(
                stream,
                410,
                "{\"error\":\"Pair request expired or declined\"}",
                "application/json",
            );
            return;
        };
        let body = if request.approved {
            let token = state.lock().map(|c| c.token.clone()).unwrap_or_default();
            requests.remove(id);
            serde_json::json!({"paired":true,"token":token}).to_string()
        } else {
            "{\"paired\":false}".to_owned()
        };
        reply(stream, 200, &body, "application/json");
        return;
    }
    if method == "OPTIONS" {
        let _=write!(stream,"HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, HEAD, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Authorization, Content-Type, Range\r\nAccess-Control-Max-Age: 600\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        return;
    }
    if path == "/api/v1/health" && method == "GET" {
        reply(stream, 200, "{\"service\":\"luma\"}", "application/json");
        return;
    }
    if let Some(id) = path
        .strip_prefix("/api/v1/media/")
        .and_then(|v| v.split('/').next()?.parse::<i64>().ok())
    {
        if !enabled {
            reply(
                stream,
                503,
                "{\"error\":\"Mobile bridge is disabled\"}",
                "application/json",
            );
            return;
        }
        let query = target.split_once('?').map(|x| x.1).unwrap_or("");
        let fields: std::collections::HashMap<_, _> =
            query.split('&').filter_map(|p| p.split_once('=')).collect();
        let exp = fields
            .get("expires")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        let sig = fields.get("signature").copied().unwrap_or("");
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let token = state.lock().map(|c| c.token.clone()).unwrap_or_default();
        if exp < now || exp > now + 21600 || !valid_signature(&token, id, exp, sig) {
            reply(
                stream,
                401,
                "{\"error\":\"Invalid or expired media URL\"}",
                "application/json",
            );
            return;
        }
        let base = format!("/api/v1/media/{id}");
        if path == format!("{base}/compatible") && matches!(method.as_str(), "GET" | "HEAD") {
            let start = match fields.get("start") {
                None => 0.0,
                Some(value) => match value.parse::<f64>() {
                    Ok(value) if value.is_finite() && (0.0..=86400.0).contains(&value) => value,
                    _ => {
                        reply(
                            stream,
                            400,
                            "{\"error\":\"Invalid start position\"}",
                            "application/json",
                        );
                        return;
                    }
                },
            };
            let quality = fields.get("quality").copied().unwrap_or("auto");
            if !matches!(quality, "auto" | "480p" | "720p" | "1080p") {
                reply(stream, 400, "Unsupported quality", "text/plain");
                return;
            }
            stream_compatible(stream, app, id, start, method == "HEAD", quality);
        } else if path == base && method == "GET" {
            stream_media(stream, app, id, &data[..split]);
        } else if let Some(index) = path
            .strip_prefix(&format!("{base}/subtitles/"))
            .and_then(|v| v.parse::<usize>().ok())
        {
            let source =
                crate::resolve_media_file(id, app.state::<media_core::LibraryState>(), app.clone());
            match source
                .and_then(|source| {
                    source
                        .subtitles
                        .get(index)
                        .map(|s| s.path.clone())
                        .ok_or("Subtitle not found".into())
                })
                .and_then(|path| std::fs::read_to_string(path).map_err(|e| e.to_string()))
            {
                Ok(text) if text.len() <= 8 * 1024 * 1024 => {
                    reply(stream, 200, &text, "text/vtt; charset=utf-8")
                }
                _ => reply(stream, 404, "Subtitle not found", "text/plain"),
            }
        } else {
            reply(stream, 404, "Not found", "text/plain");
        }
        return;
    }
    if !enabled {
        reply(
            stream,
            503,
            "{\"error\":\"Mobile bridge is disabled\"}",
            "application/json",
        );
        return;
    }
    if !authorized {
        reply(
            stream,
            401,
            "{\"error\":\"Unauthorized\"}",
            "application/json",
        );
        return;
    }
    if method != "POST" || path != "/api/v1/command" {
        reply(stream, 404, "{\"error\":\"Not found\"}", "application/json");
        return;
    }
    let request: serde_json::Value = match serde_json::from_slice(&data[split..split + length]) {
        Ok(v) => v,
        Err(_) => {
            reply(
                stream,
                400,
                "{\"error\":\"Invalid JSON\"}",
                "application/json",
            );
            return;
        }
    };
    let command = request
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let args = request
        .get("args")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    match dispatch(app, command, &args) {
        Ok(mut v) => {
            if command == "resolve_media_file" {
                if let Some(id) = args.get("mediaId").and_then(|x| x.as_i64()) {
                    let exp = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                        + 21600;
                    let token = state.lock().map(|c| c.token.clone()).unwrap_or_default();
                    let url = format!(
                        "{}/api/v1/media/{id}?expires={exp}&signature={}",
                        info(&state.lock().unwrap()).base_url.unwrap_or_default(),
                        sign(&token, id, exp)
                    );
                    if let Some(obj) = v.as_object_mut() {
                        obj.insert("path".into(), serde_json::Value::String(url.clone()));
                        obj.insert("streamUrl".into(), serde_json::Value::String(url));
                        if let Some(subtitles) =
                            obj.get_mut("subtitles").and_then(|v| v.as_array_mut())
                        {
                            for (index, subtitle) in subtitles.iter_mut().enumerate() {
                                subtitle["path"]=serde_json::Value::String(format!("{}/api/v1/media/{id}/subtitles/{index}?expires={exp}&signature={}",info(&state.lock().unwrap()).base_url.unwrap_or_default(),sign(&token,id,exp)));
                            }
                        }
                    }
                }
            }
            let body = serde_json::to_string(&v).unwrap_or_else(|_| "null".into());
            reply(stream, 200, &body, "application/json");
        }
        Err(e) => {
            let body = serde_json::json!({"error":e}).to_string();
            reply(stream, 400, &body, "application/json");
        }
    }
}
fn sign(key: &str, id: i64, exp: u64) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let mut mac = Hmac::<Sha256>::new_from_slice(key.as_bytes()).expect("HMAC accepts any key");
    mac.update(format!("{id}:{exp}").as_bytes());
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn valid_signature(key: &str, id: i64, exp: u64, sig: &str) -> bool {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let Ok(bytes) = hex_bytes(sig) else {
        return false;
    };
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(key.as_bytes()) else {
        return false;
    };
    mac.update(format!("{id}:{exp}").as_bytes());
    mac.verify_slice(&bytes).is_ok()
}
fn hex_bytes(s: &str) -> Result<Vec<u8>, ()> {
    if s.len() != 64 || !s.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| ()))
        .collect()
}
fn stream_media(s: &mut TcpStream, app: &tauri::AppHandle, id: i64, headers: &[u8]) {
    let state = app.state::<media_core::LibraryState>();
    let path = match media_core::LibraryStore::open(&state.db_path).and_then(|store| {
        store
            .resolve_media_path(id)?
            .ok_or_else(|| "Media file not found".to_owned())
    }) {
        Ok(p) => p,
        Err(_) => {
            reply(s, 404, "Not found", "text/plain");
            return;
        }
    };
    let Ok(mut file) = std::fs::File::open(&path) else {
        reply(s, 404, "Not found", "text/plain");
        return;
    };
    let Ok(meta) = file.metadata() else {
        reply(s, 404, "Not found", "text/plain");
        return;
    };
    let size = meta.len();
    let head = String::from_utf8_lossy(headers);
    let range = head.lines().find_map(|l| {
        l.to_ascii_lowercase()
            .strip_prefix("range:")
            .map(str::trim)
            .map(str::to_owned)
    });
    let parsed = byte_range(range.as_deref(), size);
    let Ok((start, end, partial)) = parsed else {
        let _=write!(s,"HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{size}\r\nContent-Length: 0\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n");
        return;
    };
    let len = if size == 0 { 0 } else { end - start + 1 };
    if file.seek(std::io::SeekFrom::Start(start)).is_err() {
        reply(s, 500, "Read failed", "text/plain");
        return;
    }
    let status = if partial { 206 } else { 200 };
    let mime = match path
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "mp4" => "video/mp4",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mp3" => "audio/mpeg",
        "m4v" => "video/x-m4v",
        _ => "application/octet-stream",
    };
    let _=write!(s,"HTTP/1.1 {status} OK\r\nContent-Type: {mime}\r\nAccept-Ranges: bytes\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Expose-Headers: Content-Range, Accept-Ranges\r\nContent-Length: {len}\r\n{}Connection: close\r\n\r\n",if partial{format!("Content-Range: bytes {start}-{end}/{size}\r\n")}else{String::new()});
    let mut left = len;
    let mut buf = [0u8; 65536];
    while left > 0 {
        let n = (left as usize).min(buf.len());
        match file.read(&mut buf[..n]) {
            Ok(0) | Err(_) => break,
            Ok(got) => {
                if s.write_all(&buf[..got]).is_err() {
                    break;
                }
                left -= got as u64;
            }
        }
    }
}
fn stream_compatible(s: &mut TcpStream, app: &tauri::AppHandle, id: i64, start: f64, head: bool, quality: &str) {
    use sha2::{Digest, Sha256};
    let state = app.state::<media_core::LibraryState>();
    let expected_path = match media_core::LibraryStore::open(&state.db_path)
        .and_then(|store| store.resolve_media_path(id))
        .and_then(|path| path.ok_or_else(|| "Media file not found".to_owned()))
        .and_then(|path| std::fs::canonicalize(path).map_err(|error| error.to_string()))
    {
        Ok(path) => path,
        Err(_) => { reply(s, 404, "Media file not found", "text/plain"); return; }
    };
    let path = expected_path.to_string_lossy();
    let normalized = path.strip_prefix(r"\\?\UNC\").map(|rest| format!(r"\\{rest}"))
        .unwrap_or_else(|| path.strip_prefix(r"\\?\").unwrap_or(&path).to_owned());
    let identity = format!("{:x}", Sha256::digest(normalized.replace('\\', "/").to_lowercase().as_bytes()));
    if let Err(error) = app
        .state::<crate::media_server::MediaServerState>()
        .request("status")
    {
        let body = serde_json::json!({"error":error}).to_string();
        reply(s, 503, &body, "application/json");
        return;
    }
    let client = match reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(6))
        .timeout(None::<std::time::Duration>)
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            let body = serde_json::json!({"error":error.to_string()}).to_string();
            reply(s, 502, &body, "application/json");
            return;
        }
    };
    let url = format!("{ADMIN_ORIGIN}/api/mobile-stream/file-{id}?start={start}&quality={quality}");
    let request = if head {
        client.head(&url)
    } else {
        client.get(&url)
    };
    let mut response = match request.header("X-Luma-Control", "1").header("X-Luma-Media-Identity", &identity).send() {
        Ok(response) => response,
        Err(error) => {
            let body = serde_json::json!({"error":format!("Could not start mobile media conversion: {error}")}).to_string();
            reply(s, 502, &body, "application/json");
            return;
        }
    };
    let status = response.status();
    if !status.is_success() {
        let code = status.as_u16();
        let mut detail = String::new();
        let _ = response
            .take(MAX_RESPONSE_BYTES)
            .read_to_string(&mut detail);
        let body = serde_json::json!({"error":detail.trim()}).to_string();
        reply(s, code, &body, "application/json");
        return;
    }
    if response.headers().get("X-Luma-Media-Identity").and_then(|value| value.to_str().ok()) != Some(identity.as_str()) {
        reply(s, 409, "{\"error\":\"The media service is outdated or selected a different file. Restart Luma before trying again.\"}", "application/json");
        return;
    }
    let Some(content_type) = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .filter(|value| {
            value
                .split(';')
                .next()
                .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("video/mp4"))
        })
    else {
        reply(
            s,
            502,
            "{\"error\":\"Unexpected media type from transcoder\"}",
            "application/json",
        );
        return;
    };
    let duration = response
        .headers()
        .get("X-Luma-Duration")
        .and_then(|value| value.to_str().ok())
        .filter(|value| value.parse::<f64>().is_ok_and(f64::is_finite));
    let code = status.as_u16();
    let reason = if code == 206 { "Partial Content" } else { "OK" };
    let mut header = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: {content_type}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Expose-Headers: X-Luma-Duration, X-Luma-Bitrate-Limit, X-Luma-Source-Bitrate\r\nConnection: close\r\n"
    );
    if let Some(duration) = duration {
        header.push_str(&format!("X-Luma-Duration: {duration}\r\n"));
    }
    for name in ["X-Luma-Bitrate-Limit", "X-Luma-Source-Bitrate"] {
        if let Some(value) = response.headers().get(name).and_then(|value| value.to_str().ok()).filter(|value| value.parse::<u64>().is_ok()) {
            header.push_str(&format!("{name}: {value}\r\n"));
        }
    }
    if let Some(length) = response.content_length() {
        header.push_str(&format!("Content-Length: {length}\r\n"));
    }
    header.push_str("\r\n");
    if s.write_all(header.as_bytes()).is_err() || head {
        return;
    }
    let _ = io::copy(&mut response, s);
}
fn byte_range(range: Option<&str>, size: u64) -> Result<(u64, u64, bool), ()> {
    let Some(range) = range else {
        return Ok((0, size.saturating_sub(1), false));
    };
    if size == 0 {
        return Err(());
    }
    let (first, last) = range
        .strip_prefix("bytes=")
        .ok_or(())?
        .split_once('-')
        .ok_or(())?;
    if first.is_empty() {
        let suffix = last.parse::<u64>().map_err(|_| ())?;
        if suffix == 0 {
            return Err(());
        }
        return Ok((size.saturating_sub(suffix), size - 1, true));
    }
    let start = first.parse::<u64>().map_err(|_| ())?;
    let end = if last.is_empty() {
        size - 1
    } else {
        last.parse::<u64>().map_err(|_| ())?.min(size - 1)
    };
    if start >= size || end < start {
        return Err(());
    }
    Ok((start, end, true))
}
fn reply(s: &mut TcpStream, status: u16, body: &str, content_type: &str) {
    let _=write!(s,"HTTP/1.1 {status} {}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n{body}",if status==200{"OK"}else{"Error"},body.len());
}
fn val<T: serde::de::DeserializeOwned>(a: &serde_json::Value, key: &str, default: T) -> T {
    a.get(key)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or(default)
}
fn dispatch(
    app: &tauri::AppHandle,
    c: &str,
    a: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    use serde::Serialize;
    fn json<T: Serialize>(x: T) -> Result<serde_json::Value, String> {
        serde_json::to_value(x).map_err(|e| e.to_string())
    }
    let r = match c {
        "get_search_browse" => {
            tauri::async_runtime::block_on(search_browse(app.state::<crate::tmdb::TmdbState>()))
        }
        "desktop_bootstrap" => json(
            serde_json::json!({"version":env!("CARGO_PKG_VERSION"),"platform":std::env::consts::OS,"mediaCoreStatus":"library-index-ready","nativeWindowFrame":true}),
        ),
        "load_remote_artwork" => json(tauri::async_runtime::block_on(
            crate::artwork::load_remote_artwork(
                val(a, "url", String::new()),
                app.state::<crate::artwork::ArtworkState>(),
            ),
        )?),
        "release_search" => release_search_proxy(app, a),
        "get_library_status" => json(crate::get_library_status(
            app.state::<media_core::LibraryState>(),
        )?),
        "get_catalog_page" => json(crate::get_catalog_page(
            val(a, "offset", 0),
            val(a, "count", 48),
            a.get("kind").and_then(|v| v.as_str()).map(str::to_owned),
            a.get("query").and_then(|v| v.as_str()).map(str::to_owned),
            a.get("sort").and_then(|v| v.as_str()).map(str::to_owned),
            app.state::<media_core::LibraryState>(),
        )?),
        "get_local_title_detail" => json(crate::get_local_title_detail(
            val(a, "mediaId", 0),
            app.state::<media_core::LibraryState>(),
            app.clone(),
        )?),
        "resolve_media_file" => json(crate::resolve_media_file(
            val(a, "mediaId", 0),
            app.state::<media_core::LibraryState>(),
            app.clone(),
        )?),
        "save_playback_progress" => {
            crate::save_playback_progress(
                val(a, "mediaId", 0),
                val(a, "positionSeconds", 0.0),
                val(a, "durationSeconds", 0.0),
                app.state::<media_core::LibraryState>(),
            )?;
            json(())
        }
        "record_playback_activity" => {
            crate::record_playback_activity(
                val(a, "mediaId", 0),
                a.get("markPreviousEpisodesWatched")
                    .and_then(|v| v.as_bool()),
                app.state::<media_core::LibraryState>(),
            )?;
            json(())
        }
        "get_playback_history" => json(crate::get_playback_history(
            val(a, "count", 12),
            app.state::<media_core::LibraryState>(),
        )?),
        "get_continue_watching" => json(crate::get_continue_watching(
            val(a, "count", 12),
            app.state::<media_core::LibraryState>(),
        )?),
        "search_tmdb" => json(tauri::async_runtime::block_on(crate::search_tmdb(
            val(a, "query", String::new()),
            app.state::<crate::tmdb::TmdbState>(),
        ))?),
        "get_discovery_feed" => json(tauri::async_runtime::block_on(
            crate::recommendations::get_discovery_feed(
                app.state::<media_core::LibraryState>(),
                app.state::<crate::tmdb::TmdbState>(),
            ),
        )?),
        "get_discovery_title" => json(tauri::async_runtime::block_on(
            crate::recommendations::get_discovery_title(
                val(a, "id", 0),
                val(a, "kind", String::new()),
                app.state::<crate::tmdb::TmdbState>(),
            ),
        )?),
        "get_discovery_trailer" => json(tauri::async_runtime::block_on(
            crate::recommendations::get_discovery_trailer(
                val(a, "id", 0),
                val(a, "kind", String::new()),
                app.state::<crate::tmdb::TmdbState>(),
            ),
        )?),
        "get_discovery_logo" => json(tauri::async_runtime::block_on(
            crate::recommendations::get_discovery_logo(
                val(a, "id", 0),
                val(a, "kind", String::new()),
                app.state::<crate::tmdb::TmdbState>(),
            ),
        )?),
        "get_title_trailer" => json(tauri::async_runtime::block_on(crate::get_title_trailer(
            val(a, "mediaId", 0),
            app.state::<media_core::LibraryState>(),
            app.state::<crate::tmdb::TmdbState>(),
        ))?),
        "get_title_logo" => json(tauri::async_runtime::block_on(crate::get_title_logo(
            val(a, "mediaId", 0),
            app.state::<media_core::LibraryState>(),
            app.state::<crate::tmdb::TmdbState>(),
        ))?),
        "discover_cast_devices" => json(discover(&app.state::<BridgeState>())?),
        "cast_media" => {
            cast(
                &app.state::<BridgeState>(),
                app,
                val(a, "deviceId", String::new()).as_str(),
                val(a, "mediaId", 0),
            )?;
            json(())
        }
        "scan_library" => json(tauri::async_runtime::block_on(crate::scan_library(
            val(a, "rootPath", String::new()),
            app.state::<media_core::LibraryState>(),
            app.state::<crate::tmdb::TmdbState>(),
        ))?),
        "rescan_library" => json(tauri::async_runtime::block_on(crate::rescan_library(
            app.state::<media_core::LibraryState>(),
            app.state::<crate::tmdb::TmdbState>(),
        ))?),
        "torrent_snapshot" => json(tauri::async_runtime::block_on(
            crate::torrent_engine::torrent_snapshot(
                app.state::<crate::torrent_engine::TorrentState>(),
            ),
        )?),
        "torrent_add_magnet" => json(tauri::async_runtime::block_on(
            crate::torrent_engine::torrent_add_magnet(
                app.state::<crate::torrent_engine::TorrentState>(),
                val(a, "uri", String::new()),
                a.get("destination")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned),
            ),
        )?),
        "torrent_add_file" => json(tauri::async_runtime::block_on(
            crate::torrent_engine::torrent_add_file(
                app.state::<crate::torrent_engine::TorrentState>(),
                val(a, "path", String::new()),
                a.get("destination")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned),
            ),
        )?),
        "torrent_add_data" => json(tauri::async_runtime::block_on(
            crate::torrent_engine::torrent_add_data(
                app.state::<crate::torrent_engine::TorrentState>(),
                val(a, "encoded", String::new()),
                a.get("destination")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned),
            ),
        )?),
        "torrent_set_paused" => {
            tauri::async_runtime::block_on(crate::torrent_engine::torrent_set_paused(
                app.state::<crate::torrent_engine::TorrentState>(),
                val(a, "infoHash", String::new()),
                val(a, "paused", false),
            ))?;
            json(())
        }
        "torrent_move_queue" => {
            tauri::async_runtime::block_on(crate::torrent_engine::torrent_move_queue(
                app.state::<crate::torrent_engine::TorrentState>(),
                val(a, "infoHash", String::new()),
                val(a, "direction", 0),
            ))?;
            json(())
        }
        "torrent_set_limits" => {
            tauri::async_runtime::block_on(crate::torrent_engine::torrent_set_limits(
                app.state::<crate::torrent_engine::TorrentState>(),
                val(a, "download", 0),
                val(a, "upload", 0),
            ))?;
            json(())
        }
        "torrent_remove" => {
            tauri::async_runtime::block_on(crate::torrent_engine::torrent_remove(
                app.state::<crate::torrent_engine::TorrentState>(),
                val(a, "infoHash", String::new()),
            ))?;
            json(())
        }
        _ => Err(format!(
            "Command is not available over the mobile bridge: {c}"
        )),
    }?;
    Ok(r)
}
async fn search_browse(
    tmdb: tauri::State<'_, crate::tmdb::TmdbState>,
) -> Result<serde_json::Value, String> {
    let mut sections = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (title, movie, series) in [
        ("Crime", 80, 80),
        ("Drama", 18, 18),
        ("Science fiction", 878, 10765),
        ("Comedy", 35, 35),
    ] {
        let (movies, shows) = tokio::join!(
            tmdb.genre_preview(movie, "movie"),
            tmdb.genre_preview(series, "series")
        );
        let movies = movies?;
        let shows = shows?;
        let mut items = Vec::new();
        for (movie, show) in movies.into_iter().zip(shows) {
            for item in [movie, show] {
                if seen.insert(format!("{}:{}", item.kind, item.id)) {
                    items.push(item);
                }
                if items.len() >= 8 {
                    break;
                }
            }
            if items.len() >= 8 {
                break;
            }
        }
        sections.push(serde_json::json!({"title":title,"items":items}));
    }
    Ok(serde_json::Value::Array(sections))
}
#[tauri::command]
pub async fn get_search_browse(
    tmdb: tauri::State<'_, crate::tmdb::TmdbState>,
) -> Result<serde_json::Value, String> {
    search_browse(tmdb).await
}
fn release_search_proxy(
    app: &tauri::AppHandle,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    app.state::<BridgeState>().ensure_release_service(app)?;
    let params = args
        .get("params")
        .and_then(|v| v.as_object())
        .ok_or_else(|| "Invalid release search request.".to_owned())?;
    let mut url = reqwest::Url::parse("http://127.0.0.1:8943/api/releases").unwrap();
    for (key, value) in params {
        let Some(value) = value.as_str() else {
            continue;
        };
        url.query_pairs_mut().append_pair(key, value);
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(95))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?;
    let response=client.get(url).header("x-luma-control","1").send().map_err(|_|"Release search service is unavailable. Start the Luma media server and configure its indexers.".to_owned())?;
    let status = response.status();
    let body = response
        .text()
        .map_err(|e| format!("Could not read release search response: {e}"))?;
    let value: serde_json::Value = serde_json::from_str(&body)
        .map_err(|_| format!("Release search returned invalid JSON (HTTP {status})."))?;
    if !status.is_success() {
        return Err(value
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("Release search failed.")
            .to_owned());
    }
    Ok(value)
}
#[tauri::command]
pub async fn release_search(
    app: tauri::AppHandle,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        release_search_proxy(&app, &serde_json::json!({"params":params}))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn media_signature_is_scoped_to_id_and_expiry() {
        let sig = sign("secret", 17, 1234);
        assert!(valid_signature("secret", 17, 1234, &sig));
        assert!(!valid_signature("secret", 18, 1234, &sig));
        assert!(!valid_signature("wrong", 17, 1234, &sig));
        assert!(!valid_signature("secret", 17, 1235, &sig));
    }
    #[test]
    fn malformed_signature_is_rejected() {
        assert!(!valid_signature("secret", 1, 1, "not-hex"));
        assert!(!valid_signature("secret", 1, 1, &"é".repeat(32)));
    }
    #[test]
    fn media_ranges_support_seeking_and_suffixes() {
        assert_eq!(byte_range(None, 100), Ok((0, 99, false)));
        assert_eq!(byte_range(Some("bytes=20-"), 100), Ok((20, 99, true)));
        assert_eq!(byte_range(Some("bytes=20-999"), 100), Ok((20, 99, true)));
        assert_eq!(byte_range(Some("bytes=-10"), 100), Ok((90, 99, true)));
        for input in [
            "bytes=100-",
            "bytes=-0",
            "bytes=20-10",
            "bytes=0-5,10-20",
            "garbage",
        ] {
            assert!(byte_range(Some(input), 100).is_err());
        }
    }
    #[test]
    fn cast_targets_stay_on_the_lan() {
        assert!(private_http_url("http://192.168.1.20:1400/control"));
        assert!(!private_http_url("http://example.com/control"));
        assert!(!private_http_url("http://8.8.8.8/control"));
        assert!(!private_http_url(
            "http://user:password@192.168.1.20/control"
        ));
    }
}
