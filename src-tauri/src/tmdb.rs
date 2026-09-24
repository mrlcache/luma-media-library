use std::path::PathBuf;
use std::time::Duration;

use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};

const API_BASE: &str = "https://api.themoviedb.org/3";
const IMAGE_BASE: &str = "https://image.tmdb.org/t/p";
const MAX_RESULTS: usize = 20;

#[derive(Clone)]
pub struct TmdbState {
    token_path: PathBuf,
    client: Client,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TmdbSearchResult {
    pub id: u64,
    pub title: String,
    pub kind: String,
    pub year: Option<u16>,
    pub overview: String,
    pub vote_average: Option<f32>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
}

#[derive(Deserialize)]
struct SearchResponse {
    #[serde(default)]
    results: Vec<RawSearchResult>,
}

#[derive(Deserialize)]
struct RawSearchResult {
    id: u64,
    #[serde(default)]
    media_type: String,
    title: Option<String>,
    name: Option<String>,
    release_date: Option<String>,
    first_air_date: Option<String>,
    overview: Option<String>,
    vote_average: Option<f32>,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
}

#[derive(Deserialize)]
struct ConfigurationResponse {
    images: ImageConfiguration,
}

#[derive(Deserialize)]
struct ImageConfiguration {
    secure_base_url: String,
}

impl TmdbState {
    pub fn new(token_path: PathBuf) -> Result<Self, String> {
        static TLS_PROVIDER: std::sync::Once = std::sync::Once::new();
        TLS_PROVIDER.call_once(|| {
            let _ = rustls::crypto::ring::default_provider().install_default();
        });

        let client = Client::builder()
            .timeout(Duration::from_secs(12))
            .user_agent(concat!("Media Library/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| "Could not initialize the TMDb connection.".to_owned())?;

        Ok(Self { token_path, client })
    }

    pub async fn test_connection(&self) -> Result<(), String> {
        let token = self.read_token()?;
        let response = self
            .client
            .get(format!("{API_BASE}/configuration"))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|_| "Could not reach TMDb. Check your connection and try again.".to_owned())?;
        ensure_success(response.status())?;
        let configuration = response
            .json::<ConfigurationResponse>()
            .await
            .map_err(|_| "TMDb returned an unreadable configuration response.".to_owned())?;
        if configuration.images.secure_base_url.starts_with("https://") {
            Ok(())
        } else {
            Err("TMDb returned an invalid image configuration.".to_owned())
        }
    }

    pub async fn search(&self, query: &str) -> Result<Vec<TmdbSearchResult>, String> {
        self.search_endpoint(query, "multi", None).await
    }

    pub async fn search_for_kind(
        &self,
        query: &str,
        kind: &str,
    ) -> Result<Vec<TmdbSearchResult>, String> {
        let (endpoint, forced_kind) = match kind {
            "movie" => ("movie", "movie"),
            "series" => ("tv", "series"),
            _ => return Err("Choose a movie or series metadata type.".to_owned()),
        };
        self.search_endpoint(query, endpoint, Some(forced_kind))
            .await
    }

    async fn search_endpoint(
        &self,
        query: &str,
        endpoint: &str,
        forced_kind: Option<&str>,
    ) -> Result<Vec<TmdbSearchResult>, String> {
        let query = query.trim();
        if query.chars().count() < 2 {
            return Ok(Vec::new());
        }
        if query.chars().count() > 120 {
            return Err("Search terms must be 120 characters or fewer.".to_owned());
        }

        let token = self.read_token()?;
        let response = self
            .client
            .get(format!("{API_BASE}/search/{endpoint}"))
            .bearer_auth(token)
            .query(&[
                ("query", query),
                ("include_adult", "false"),
                ("language", "en-US"),
                ("page", "1"),
            ])
            .send()
            .await
            .map_err(|_| "Could not reach TMDb. Check your connection and try again.".to_owned())?;
        ensure_success(response.status())?;

        let response = response
            .json::<SearchResponse>()
            .await
            .map_err(|_| "TMDb returned an unreadable search response.".to_owned())?;

        Ok(response
            .results
            .into_iter()
            .filter_map(|item| {
                let kind = forced_kind.or_else(|| match item.media_type.as_str() {
                    "movie" => Some("movie"),
                    "tv" => Some("series"),
                    _ => None,
                })?;
                let title = item
                    .title
                    .or(item.name)
                    .filter(|title| !title.trim().is_empty())?;
                let date = item
                    .release_date
                    .as_deref()
                    .or(item.first_air_date.as_deref());
                let year = date
                    .and_then(|date| date.get(..4))
                    .and_then(|year| year.parse::<u16>().ok());

                Some(TmdbSearchResult {
                    id: item.id,
                    title,
                    kind: kind.to_owned(),
                    year,
                    overview: item.overview.unwrap_or_default(),
                    vote_average: item.vote_average,
                    poster_url: image_url(item.poster_path.as_deref(), "w342"),
                    backdrop_url: image_url(item.backdrop_path.as_deref(), "w780"),
                })
            })
            .take(MAX_RESULTS)
            .collect())
    }

    fn read_token(&self) -> Result<String, String> {
        let token = std::fs::read_to_string(&self.token_path)
            .map_err(|_| "TMDb credentials are not available in this app profile.".to_owned())?;
        let token = token.trim();
        if token.is_empty() {
            return Err("TMDb credentials are empty. Update the local app credential.".to_owned());
        }
        Ok(token.to_owned())
    }
}

fn image_url(path: Option<&str>, size: &str) -> Option<String> {
    let path = path?.strip_prefix('/')?;
    if path.is_empty() || path.contains("..") || path.contains(['?', '#', ':', '\\']) {
        return None;
    }
    Some(format!("{IMAGE_BASE}/{size}/{path}"))
}

fn ensure_success(status: StatusCode) -> Result<(), String> {
    if status.is_success() {
        return Ok(());
    }
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            Err("TMDb rejected the local credential. Update it in the app profile.".to_owned())
        }
        StatusCode::TOO_MANY_REQUESTS => {
            Err("TMDb rate limited this request. Wait a moment and try again.".to_owned())
        }
        _ => Err(format!(
            "TMDb returned an error (HTTP {}).",
            status.as_u16()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::image_url;

    #[test]
    fn image_paths_are_built_on_the_tmdb_image_host() {
        assert_eq!(
            image_url(Some("/poster.jpg"), "w342").as_deref(),
            Some("https://image.tmdb.org/t/p/w342/poster.jpg")
        );
        assert_eq!(image_url(None, "w342"), None);
        assert_eq!(image_url(Some("/../secret"), "w342"), None);
        assert_eq!(
            image_url(Some("https://attacker.invalid/image"), "w342"),
            None
        );
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "requires the local TMDb credential and network access"]
    fn local_credential_can_search_tmdb() {
        let token_path = std::path::PathBuf::from(
            std::env::var_os("APPDATA").expect("Windows app data directory"),
        )
        .join("local.media.platform")
        .join("tmdb_read_access_token");
        let state = super::TmdbState::new(token_path).expect("TMDb client");
        let results = tauri::async_runtime::block_on(state.search("The Matrix"))
            .expect("authenticated TMDb search");

        assert!(results.iter().any(|item| item.title == "The Matrix"));
    }
}
