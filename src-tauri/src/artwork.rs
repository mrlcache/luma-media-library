use base64::Engine as _;
use reqwest::{header::CONTENT_TYPE, redirect::Policy, Client, Url};
use std::collections::HashMap;
use std::sync::{Mutex, atomic::{AtomicBool, Ordering}};

const MAX_ARTWORK_BYTES: usize = 8 * 1024 * 1024;
const MAX_CACHE_BYTES: usize = 48 * 1024 * 1024;

pub struct ArtworkState {
    client: Client,
    cache: Mutex<HashMap<String, String>>,
    cache_enabled: AtomicBool,
}

impl ArtworkState {
    pub fn release_desktop_cache(&self) {
        self.cache_enabled.store(false, Ordering::Release);
        if let Ok(mut cache) = self.cache.lock() { cache.clear(); cache.shrink_to_fit(); }
    }

    pub fn resume_desktop_cache(&self) { self.cache_enabled.store(true, Ordering::Release); }

    pub fn new() -> Result<Self, String> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let client = Client::builder()
            .user_agent(concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")))
            .redirect(Policy::custom(|attempt| {
                if attempt.previous().len() >= 4 || !is_allowed_host(attempt.url().host_str()) {
                    attempt.stop()
                } else {
                    attempt.follow()
                }
            }))
            .build()
            .map_err(|error| format!("Could not prepare artwork requests: {error}"))?;
        Ok(Self { client, cache: Mutex::new(HashMap::new()), cache_enabled: AtomicBool::new(true) })
    }
}

fn is_allowed_host(host: Option<&str>) -> bool {
    matches!(host, Some("image.tmdb.org" | "static.tvmaze.com" | "images.unsplash.com"))
}

#[tauri::command]
pub async fn load_remote_artwork(
    url: String,
    state: tauri::State<'_, ArtworkState>,
) -> Result<String, String> {
    let parsed = Url::parse(&url).map_err(|_| "The artwork URL is invalid.".to_owned())?;
    if parsed.scheme() != "https"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_some()
        || !is_allowed_host(parsed.host_str())
    {
        return Err("Artwork can only be loaded from approved image providers.".to_owned());
    }

    if let Some(image) = state.cache.lock().map_err(|_| "The artwork cache is unavailable.".to_owned())?.get(&url) {
        return Ok(image.clone());
    }

    let response = state.client.get(parsed).send().await
        .map_err(|error| format!("Could not download artwork: {error}"))?
        .error_for_status()
        .map_err(|error| format!("The artwork provider returned an error: {error}"))?;
    let content_type = response.headers().get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .filter(|value| matches!(*value, "image/jpeg" | "image/png" | "image/webp" | "image/gif" | "image/avif"))
        .ok_or_else(|| "The artwork provider did not return a supported image.".to_owned())?
        .to_owned();
    if response.content_length().is_some_and(|length| length > MAX_ARTWORK_BYTES as u64) {
        return Err("The artwork image is too large.".to_owned());
    }

    let bytes = response.bytes().await.map_err(|error| format!("Could not read artwork: {error}"))?;
    if bytes.is_empty() || bytes.len() > MAX_ARTWORK_BYTES {
        return Err("The artwork image is empty or too large.".to_owned());
    }
    let data_url = format!("data:{content_type};base64,{}", base64::engine::general_purpose::STANDARD.encode(&bytes));
    let mut cache = state.cache.lock().map_err(|_| "The artwork cache is unavailable.".to_owned())?;
    let cache_bytes: usize = cache.values().map(String::len).sum();
    if state.cache_enabled.load(Ordering::Acquire) && cache_bytes + data_url.len() <= MAX_CACHE_BYTES {
        cache.insert(url, data_url.clone());
    }
    Ok(data_url)
}

#[cfg(test)]
mod tests {
    use super::is_allowed_host;

    #[test]
    fn native_client_initializes_without_a_preconfigured_tls_provider() {
        assert!(super::ArtworkState::new().is_ok());
    }

    #[test]
    fn only_known_artwork_hosts_can_be_proxied() {
        assert!(is_allowed_host(Some("image.tmdb.org")));
        assert!(is_allowed_host(Some("static.tvmaze.com")));
        assert!(!is_allowed_host(Some("example.com")));
        assert!(!is_allowed_host(Some("image.tmdb.org.example.com")));
    }
}
