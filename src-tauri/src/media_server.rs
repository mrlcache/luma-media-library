use serde_json::Value;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const ADMIN_ORIGIN: &str = "http://127.0.0.1:8940";
const STATUS_URL: &str = "http://127.0.0.1:8940/api/status";
const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;

// Windows resource paths may use the verbatim prefix. Node's entry-point
// resolver rejects that prefix, although Rust accepts it for filesystem access.
fn subprocess_path(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let text = path.to_string_lossy();
        if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{rest}"));
        }
        if let Some(rest) = text.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
    }
    path
}

#[derive(Clone)]
pub struct MediaServerState(Arc<Mutex<Supervisor>>);

impl MediaServerState {
    pub fn new(app_data_dir: PathBuf, resource_dir: PathBuf) -> Self {
        Self(Arc::new(Mutex::new(Supervisor {
            app_data_dir: subprocess_path(app_data_dir),
            resource_dir: subprocess_path(resource_dir),
            child: None,
            client: reqwest::blocking::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(6))
                .build()
                .expect("valid media server HTTP client configuration"),
        })))
    }

    pub fn request(&self, action: &str) -> Result<Value, String> {
        let mut supervisor = self.0.lock().map_err(|_| "Media server control is unavailable.".to_owned())?;
        supervisor.request(action)
    }
}

struct Supervisor {
    app_data_dir: PathBuf,
    resource_dir: PathBuf,
    child: Option<Child>,
    client: reqwest::blocking::Client,
}

impl Supervisor {
    fn request(&mut self, action: &str) -> Result<Value, String> {
        let path = match action {
            "status" => "/api/status",
            "start" => "/api/start",
            "stop" => "/api/stop",
            _ => return Err("Unsupported media server action.".to_owned()),
        };

        let current = self.ensure_available()?;
        if action == "status" {
            return Ok(current);
        }

        let response = self.client
            .post(format!("{ADMIN_ORIGIN}{path}"))
            .header(reqwest::header::ORIGIN, ADMIN_ORIGIN)
            .header("X-Luma-Control", "1")
            .send()
            .map_err(|error| format!("Could not contact the media server: {error}"))?;
        let value = parse_response(response)?;
        validate_status(&value)?;
        Ok(value)
    }

    fn ensure_available(&mut self) -> Result<Value, String> {
        self.reap_exited_child()?;
        if self.child.is_none() {
            if let Some(status) = self.probe()? {
                return Ok(status);
            }
            self.launch()?;
        }

        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if let Some(child) = self.child.as_mut() {
                if child.try_wait().map_err(|error| format!("Could not check the media server process: {error}"))?.is_some() {
                    self.child = None;
                    // Another instance may have won the port race after the
                    // first probe. Reuse it only when it speaks our API.
                    if let Some(status) = self.probe()? {
                        return Ok(status);
                    }
                    return Err(self.startup_error());
                }
            }

            match self.probe() {
                Ok(Some(status)) => return Ok(status),
                Ok(None) => {}
                Err(error) => {
                    if Instant::now() >= deadline {
                        return Err(error);
                    }
                }
            }

            if Instant::now() >= deadline {
                return Err(self.startup_error());
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    fn probe(&self) -> Result<Option<Value>, String> {
        match self.client.get(STATUS_URL).send() {
            Ok(response) => {
                let value = parse_response(response)?;
                validate_status(&value)?;
                Ok(Some(value))
            }
            Err(error) if error.is_connect() => Ok(None),
            Err(error) => Err(format!("The media server at 127.0.0.1:8940 did not respond: {error}")),
        }
    }

    fn launch(&mut self) -> Result<(), String> {
        let runtime_dir = self.app_data_dir.join("media-server");
        let db_path = self.app_data_dir.join("media-library.sqlite3");
        let service_root = self.resource_dir.join("release-service");
        let node = service_root.join("node.exe");
        let entry = service_root.join("media-server").join("src").join("main.mjs");
        if !node.is_file() || !entry.is_file() {
            return Err("The installed media server runtime is missing from the application resources.".to_owned());
        }
        std::fs::create_dir_all(&runtime_dir)
            .map_err(|error| format!("Could not prepare media server data: {error}"))?;
        let log_path = runtime_dir.join("media-server.log");
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|error| format!("Could not open the media server log: {error}"))?;
        let log_copy = log.try_clone().map_err(|error| format!("Could not prepare the media server log: {error}"))?;
        let mut command = Command::new(node);
        command
            .arg(entry)
            .arg("--db").arg(&db_path)
            .arg("--admin-port").arg("8940")
            .arg("--port").arg("8941")
            .current_dir(service_root.join("media-server"))
            .env("LUMA_SERVER_RUNTIME", &runtime_dir)
            .env("LUMA_SERVER_DB", &db_path)
            .env("LUMA_APP_DATA", &self.app_data_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log_copy))
            .stderr(Stdio::from(log));
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        self.child = Some(command.spawn().map_err(|error| format!("Could not start the media server: {error}"))?);
        Ok(())
    }

    fn reap_exited_child(&mut self) -> Result<(), String> {
        let exited = match self.child.as_mut() {
            Some(child) => child.try_wait()
                .map_err(|error| format!("Could not check the media server process: {error}"))?
                .is_some(),
            None => false,
        };
        if exited { self.child = None; }
        Ok(())
    }

    fn startup_error(&self) -> String {
        let log = self.app_data_dir.join("media-server").join("media-server.log");
        format!("The media server did not become ready. Details are in {}.", log.display())
    }
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        let Some(mut child) = self.child.take() else { return; };
        let still_running = child.try_wait().map(|result| result.is_none()).unwrap_or(false);
        if still_running {
            if let Ok(response) = self.client
                .post(format!("{ADMIN_ORIGIN}/api/stop"))
                .header(reqwest::header::ORIGIN, ADMIN_ORIGIN)
                .header("X-Luma-Control", "1")
                .send()
            {
                let _ = parse_response(response);
            }
            let _ = child.kill();
        }
        let _ = child.wait();
    }
}

fn parse_response(response: reqwest::blocking::Response) -> Result<Value, String> {
    let status = response.status();
    if response.content_length().is_some_and(|length| length > MAX_RESPONSE_BYTES) {
        return Err("The media server returned an oversized response.".to_owned());
    }
    let mut limited = response.take(MAX_RESPONSE_BYTES + 1);
    let mut body = Vec::new();
    limited.read_to_end(&mut body).map_err(|error| format!("Could not read the media server response: {error}"))?;
    if body.len() as u64 > MAX_RESPONSE_BYTES {
        return Err("The media server returned an oversized response.".to_owned());
    }
    let value: Value = serde_json::from_slice(&body)
        .map_err(|_| format!("Unexpected response from the media server (HTTP {}).", status.as_u16()))?;
    if !status.is_success() {
        return Err(value.get("error").and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| format!("Media server request failed with HTTP {}.", status.as_u16())));
    }
    Ok(value)
}

fn validate_status(value: &Value) -> Result<(), String> {
    let valid = value.get("running").and_then(Value::as_bool).is_some()
        && value.get("name").and_then(Value::as_str).is_some()
        && value.get("url").and_then(Value::as_str).is_some()
        && value.get("items").and_then(Value::as_u64).is_some();
    if valid {
        Ok(())
    } else {
        Err("Port 8940 is occupied by a service that is not the Luma media server.".to_owned())
    }
}
