use std::path::PathBuf;
use std::time::Duration;

use reqwest::{Client, StatusCode, Url};
use serde::{Deserialize, Serialize};
use tauri::Manager;

const API_BASE: &str = "https://api.opensubtitles.com/api/v1";
const USER_AGENT: &str = concat!("Media Library v", env!("CARGO_PKG_VERSION"));

pub struct OpenSubtitlesState {
    data_dir: PathBuf,
    client: Client,
    token: std::sync::Mutex<Option<String>>,
    api_key: std::sync::Mutex<Option<String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleSearchResult {
    pub file_id: i64,
    pub release: String,
    pub language: String,
    pub downloads: u64,
    pub hearing_impaired: bool,
    pub uploader: Option<String>,
}

#[derive(Deserialize)]
struct SearchResponse {
    #[serde(default)]
    data: Vec<SearchItem>,
}

#[derive(Deserialize)]
struct SearchItem {
    attributes: SearchAttributes,
}

#[derive(Deserialize)]
struct SearchAttributes {
    #[serde(default)]
    language: String,
    release: Option<String>,
    #[serde(default)]
    download_count: u64,
    #[serde(default)]
    hearing_impaired: bool,
    uploader: Option<Uploader>,
    #[serde(default)]
    files: Vec<SubtitleFile>,
}

#[derive(Deserialize)]
struct Uploader {
    name: Option<String>,
}

#[derive(Deserialize)]
struct SubtitleFile {
    file_id: i64,
    file_name: Option<String>,
}

#[derive(Deserialize)]
struct LoginResponse {
    token: String,
}

#[derive(Deserialize)]
struct DownloadResponse {
    link: String,
}

impl OpenSubtitlesState {
    pub fn new(data_dir: PathBuf) -> Result<Self, String> {
        static TLS_PROVIDER: std::sync::Once = std::sync::Once::new();
        TLS_PROVIDER.call_once(|| {
            let _ = rustls::crypto::ring::default_provider().install_default();
        });
        let client = Client::builder()
            .timeout(Duration::from_secs(20))
            .user_agent(USER_AGENT)
            .build()
            .map_err(|_| "Could not initialize the OpenSubtitles connection.".to_owned())?;
        Ok(Self {
            data_dir,
            client,
            token: std::sync::Mutex::new(None),
            api_key: std::sync::Mutex::new(None),
        })
    }

    pub fn set_api_key(&self, api_key: &str) -> Result<(), String> {
        let api_key = api_key.trim();
        if api_key.len() < 8 || api_key.len() > 512 || api_key.chars().any(char::is_whitespace) {
            return Err("Enter a valid OpenSubtitles API key.".to_owned());
        }
        *self
            .api_key
            .lock()
            .map_err(|_| "OpenSubtitles settings are unavailable.".to_owned())? =
            Some(api_key.to_owned());
        *self
            .token
            .lock()
            .map_err(|_| "OpenSubtitles session is unavailable.".to_owned())? = None;
        Ok(())
    }

    pub async fn login(&self, username: &str, password: &str) -> Result<(), String> {
        let api_key = self.api_key()?;
        let username = username.trim();
        if username.is_empty() || password.is_empty() {
            return Err(
                "Enter your OpenSubtitles username and password to download a subtitle.".to_owned(),
            );
        }
        let response = self
            .client
            .post(format!("{API_BASE}/login"))
            .header("Api-Key", api_key)
            .header("User-Agent", USER_AGENT)
            .json(&serde_json::json!({ "username": username, "password": password }))
            .send()
            .await
            .map_err(|_| {
                "Could not reach OpenSubtitles. Check your connection and try again.".to_owned()
            })?;
        if !response.status().is_success() {
            return Err(api_error(response.status(), "OpenSubtitles login failed"));
        }
        let login = response
            .json::<LoginResponse>()
            .await
            .map_err(|_| "OpenSubtitles returned an invalid sign-in response.".to_owned())?;
        if login.token.trim().is_empty() {
            return Err("OpenSubtitles did not provide a session token.".to_owned());
        }
        *self
            .token
            .lock()
            .map_err(|_| "OpenSubtitles session could not be stored.".to_owned())? =
            Some(login.token);
        Ok(())
    }

    pub async fn search(
        &self,
        query: &str,
        language: &str,
        year: Option<u16>,
        kind: &str,
    ) -> Result<Vec<SubtitleSearchResult>, String> {
        let query = query.trim();
        if query.chars().count() < 2 || query.chars().count() > 120 {
            return Err("Subtitle search needs a title between 2 and 120 characters.".to_owned());
        }
        if !matches!(kind, "movie" | "series") {
            return Err("Choose a movie or series type to search subtitles.".to_owned());
        }
        if language.len() < 2
            || language.len() > 12
            || !language
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return Err("Choose a valid subtitle language.".to_owned());
        }
        let api_key = self.api_key()?;
        let mut request = self
            .client
            .get(format!("{API_BASE}/subtitles"))
            .header("Api-Key", api_key)
            .header("User-Agent", USER_AGENT)
            .query(&[
                ("query", query),
                ("languages", language),
                ("type", if kind == "series" { "episode" } else { "movie" }),
            ]);
        if let Some(year) = year {
            request = request.query(&[("year", year.to_string())]);
        }
        let response = request.send().await.map_err(|_| {
            "Could not reach OpenSubtitles. Check your connection and try again.".to_owned()
        })?;
        if !response.status().is_success() {
            return Err(api_error(response.status(), "OpenSubtitles search failed"));
        }
        let response = response
            .json::<SearchResponse>()
            .await
            .map_err(|_| "OpenSubtitles returned an invalid search response.".to_owned())?;
        let mut results = Vec::new();
        for item in response.data {
            let attributes = item.attributes;
            let uploader = attributes.uploader.and_then(|uploader| uploader.name);
            for file in attributes.files {
                if file.file_id <= 0 {
                    continue;
                }
                results.push(SubtitleSearchResult {
                    file_id: file.file_id,
                    release: attributes
                        .release
                        .clone()
                        .or(file.file_name)
                        .unwrap_or_else(|| "Subtitle file".to_owned()),
                    language: attributes.language.clone(),
                    downloads: attributes.download_count,
                    hearing_impaired: attributes.hearing_impaired,
                    uploader: uploader.clone(),
                });
            }
        }
        Ok(results.into_iter().take(30).collect())
    }

    pub async fn download(
        &self,
        media_id: i64,
        file_id: i64,
        app: &tauri::AppHandle,
    ) -> Result<String, String> {
        if media_id <= 0 || file_id <= 0 {
            return Err("This subtitle selection is invalid.".to_owned());
        }
        let api_key = self.api_key()?;
        let token = self
            .token
            .lock()
            .map_err(|_| "OpenSubtitles session is unavailable.".to_owned())?
            .clone()
            .ok_or_else(|| {
                "Sign in to OpenSubtitles in this panel before downloading.".to_owned()
            })?;
        let response = self
            .client
            .post(format!("{API_BASE}/download"))
            .header("Api-Key", api_key)
            .bearer_auth(token)
            .header("User-Agent", USER_AGENT)
            .json(&serde_json::json!({ "file_id": file_id, "sub_format": "srt" }))
            .send()
            .await
            .map_err(|_| "Could not request this subtitle from OpenSubtitles.".to_owned())?;
        if !response.status().is_success() {
            return Err(api_error(
                response.status(),
                "OpenSubtitles download request failed",
            ));
        }
        let download = response
            .json::<DownloadResponse>()
            .await
            .map_err(|_| "OpenSubtitles returned an invalid download response.".to_owned())?;
        let url = Url::parse(&download.link)
            .map_err(|_| "OpenSubtitles returned an invalid subtitle link.".to_owned())?;
        let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
        if url.scheme() != "https"
            || !(host == "opensubtitles.com" || host.ends_with(".opensubtitles.com"))
        {
            return Err(
                "OpenSubtitles returned a download link outside its secure domain.".to_owned(),
            );
        }
        let response = self
            .client
            .get(url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await
            .map_err(|_| "The subtitle file could not be downloaded.".to_owned())?;
        if !response.status().is_success() {
            return Err(api_error(
                response.status(),
                "Subtitle file download failed",
            ));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|_| "The subtitle file could not be read.".to_owned())?;
        if bytes.len() > 8 * 1024 * 1024 {
            return Err("This subtitle file is unexpectedly large.".to_owned());
        }
        let text = String::from_utf8_lossy(&bytes)
            .trim_start_matches('\u{feff}')
            .to_owned();
        if !text.contains("-->") {
            return Err("The downloaded file is not a supported SRT/VTT subtitle.".to_owned());
        }
        let vtt = if text.trim_start().starts_with("WEBVTT") {
            text
        } else {
            super::srt_to_webvtt(&text)
        };
        let subtitle_dir = self.data_dir.join("subtitles");
        std::fs::create_dir_all(&subtitle_dir)
            .map_err(|_| "Could not create the local subtitle cache.".to_owned())?;
        let path = subtitle_dir.join(format!("{media_id}-{file_id}.vtt"));
        std::fs::write(&path, vtt)
            .map_err(|_| "Could not save the downloaded subtitle locally.".to_owned())?;
        app.asset_protocol_scope()
            .allow_file(&path)
            .map_err(|_| "Could not authorize the downloaded subtitle for playback.".to_owned())?;
        Ok(path.to_string_lossy().to_string())
    }

    fn api_key(&self) -> Result<String, String> {
        self.api_key
            .lock()
            .map_err(|_| "OpenSubtitles settings are unavailable.".to_owned())?
            .clone()
            .ok_or_else(|| {
                "Enter your OpenSubtitles API key in the subtitle panel for this session."
                    .to_owned()
            })
    }
}

fn api_error(status: StatusCode, action: &str) -> String {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            format!("{action}: OpenSubtitles rejected the API key or account session.")
        }
        StatusCode::TOO_MANY_REQUESTS => {
            format!("{action}: OpenSubtitles reached its request/download limit. Try again later.")
        }
        _ => format!("{action} (HTTP {}).", status.as_u16()),
    }
}
