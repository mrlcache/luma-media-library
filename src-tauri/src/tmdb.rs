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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TmdbTrailer {
    pub key: String,
    pub name: String,
    pub is_teaser: bool,
}

#[derive(Deserialize)]
struct ImagesResponse {
    #[serde(default)]
    logos: Vec<RawLogo>,
}

#[derive(Deserialize)]
struct RawLogo {
    file_path: String,
    iso_639_1: Option<String>,
    #[serde(default)]
    vote_average: f32,
    #[serde(default)]
    width: u32,
}

#[derive(Deserialize)]
struct VideoResponse {
    #[serde(default)]
    results: Vec<RawVideo>,
}

#[derive(Clone, Deserialize)]
struct RawVideo {
    key: String,
    name: String,
    site: String,
    #[serde(rename = "type")]
    video_type: String,
    #[serde(default)]
    official: bool,
    #[serde(default)]
    size: u16,
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
    #[serde(default)]
    adult: bool,
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
    pub async fn genre_preview(&self, genre:u64, kind:&str) -> Result<Vec<TmdbSearchResult>,String> {
        let endpoint=media_endpoint(kind)?;
        let response=self.client.get(format!("{API_BASE}/discover/{endpoint}"))
            .bearer_auth(self.read_token()?).query(&[("language","en-US"),("include_adult","false"),("sort_by","popularity.desc"),("with_genres",&genre.to_string())])
            .send().await.map_err(|_|"Could not load genres from TMDb.".to_owned())?;
        ensure_success(response.status())?;
        let response=response.json::<SearchResponse>().await.map_err(|_|"TMDb returned unreadable genre results.".to_owned())?;
        Ok(response.results.into_iter().filter(|item|!item.adult).filter_map(|item|discovery_item(item,Some(kind))).take(8).collect())
    }
    pub async fn discovery_list(&self, endpoint: &str, kind: Option<&str>) -> Result<Vec<TmdbSearchResult>, String> {
        self.discovery_list_page(endpoint, kind, 1).await
    }

    pub async fn discovery_list_page(&self, endpoint: &str, kind: Option<&str>, page: u32) -> Result<Vec<TmdbSearchResult>, String> {
        let response = self.client.get(format!("{API_BASE}/{endpoint}"))
            .bearer_auth(self.read_token()?).query(&[("language", "en-US"), ("include_adult", "false"), ("page", &page.to_string())])
            .send().await.map_err(|_| "Could not load recommendations from TMDb.".to_owned())?;
        ensure_success(response.status())?;
        let response = response.json::<SearchResponse>().await.map_err(|_| "TMDb returned unreadable recommendations.".to_owned())?;
        Ok(response.results.into_iter().filter(|item| !item.adult).filter_map(|item| discovery_item(item, kind)).collect())
    }

    pub async fn discovery_title(&self, id: u64, kind: &str) -> Result<TmdbSearchResult, String> {
        let endpoint = media_endpoint(kind)?;
        let response = self.client.get(format!("{API_BASE}/{endpoint}/{id}"))
            .bearer_auth(self.read_token()?).query(&[("language", "en-US")])
            .send().await.map_err(|_| "Could not load this title from TMDb.".to_owned())?;
        ensure_success(response.status())?;
        let item = response.json::<RawSearchResult>().await.map_err(|_| "TMDb returned unreadable title metadata.".to_owned())?;
        discovery_item(item, Some(kind)).ok_or_else(|| "Title not found.".to_owned())
    }
    pub fn new(token_path: PathBuf) -> Result<Self, String> {
        static TLS_PROVIDER: std::sync::Once = std::sync::Once::new();
        TLS_PROVIDER.call_once(|| {
            let _ = rustls::crypto::ring::default_provider().install_default();
        });

        let client = Client::builder()
            .timeout(Duration::from_secs(12))
            .user_agent(concat!("Luma/", env!("CARGO_PKG_VERSION")))
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

    pub async fn trailer_for_kind(
        &self,
        id: u64,
        kind: &str,
    ) -> Result<Option<TmdbTrailer>, String> {
        let endpoint = match kind {
            "movie" => "movie",
            "series" => "tv",
            _ => return Ok(None),
        };
        let token = self.read_token()?;
        let mut teasers = Vec::new();
        for language in [Some("en-US"), Some("pt-BR"), None] {
            let mut request = self.client.get(format!("{API_BASE}/{endpoint}/{id}/videos")).bearer_auth(&token);
            if let Some(language) = language { request = request.query(&[("language", language)]); }
            let response = request.send().await.map_err(|_| "Could not reach TMDb for this title's trailer.".to_owned())?;
            ensure_success(response.status())?;
            let videos = response.json::<VideoResponse>().await.map_err(|_| "TMDb returned unreadable trailer metadata.".to_owned())?;
            teasers.extend(videos.results.iter().filter(|video| video.video_type == "Teaser").cloned());
            if let Some(trailer) = choose_trailer(videos.results) { return Ok(Some(trailer)); }
        }
        Ok(choose_video(teasers, true))
    }

    pub async fn trailer_for_title(&self, title: &str, year: Option<u16>, kind: &str) -> Result<Option<TmdbTrailer>, String> {
        let query = clean_title_query(title);
        let results = self.search_for_kind(&query, kind).await?;
        let expected = normalize_title(&query);
        let mut candidates = results.into_iter().collect::<Vec<_>>();
        candidates.sort_by_key(|result| {
            let actual = normalize_title(&result.title);
            let exact = actual == expected;
            let related = actual.len() >= 4 && (expected.starts_with(actual.as_str()) || actual.starts_with(expected.as_str()));
            let year_distance = match (result.year, year) {
                (Some(actual), Some(expected)) => actual.abs_diff(expected),
                _ => u16::MAX,
            };
            (!exact, !related, year_distance)
        });
        for candidate in candidates {
            let actual = normalize_title(&candidate.title);
            let related = actual == expected || (actual.len() >= 4 && (expected.starts_with(actual.as_str()) || actual.starts_with(expected.as_str())));
            if !related { continue; }
            if let Some(expected_year) = year {
                if candidate.year.is_some_and(|actual| actual.abs_diff(expected_year) > 1) { continue; }
            }
            if let Some(trailer) = self.trailer_for_kind(candidate.id, kind).await? { return Ok(Some(trailer)); }
        }
        Ok(None)
    }

    pub async fn logo_for_kind(&self, id: u64, kind: &str) -> Result<Option<String>, String> {
        let endpoint = match kind {
            "movie" => "movie",
            "series" => "tv",
            _ => return Ok(None),
        };
        let response = self
            .client
            .get(format!("{API_BASE}/{endpoint}/{id}/images"))
            .bearer_auth(self.read_token()?)
            .send()
            .await
            .map_err(|_| "Could not reach TMDb for this title's logo.".to_owned())?;
        ensure_success(response.status())?;
        let images = response
            .json::<ImagesResponse>()
            .await
            .map_err(|_| "TMDb returned unreadable logo metadata.".to_owned())?;
        Ok(choose_png_logo(images.logos))
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
                    poster_url: image_url(item.poster_path.as_deref(), "w780"),
                    backdrop_url: image_url(item.backdrop_path.as_deref(), "w1280"),
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

pub fn media_endpoint(kind: &str) -> Result<&'static str, String> {
    match kind { "movie" => Ok("movie"), "series" => Ok("tv"), _ => Err("Choose a movie or series.".to_owned()) }
}

fn discovery_item(item: RawSearchResult, forced_kind: Option<&str>) -> Option<TmdbSearchResult> {
    if item.adult { return None; }
    let kind = forced_kind.or_else(|| match item.media_type.as_str() { "movie" => Some("movie"), "tv" => Some("series"), _ => None })?;
    let title = item.title.or(item.name).filter(|title| !title.trim().is_empty())?;
    let date = item.release_date.as_deref().or(item.first_air_date.as_deref());
    Some(TmdbSearchResult { id: item.id, title, kind: kind.to_owned(), year: date.and_then(|value| value.get(..4)).and_then(|value| value.parse().ok()),
        overview: item.overview.unwrap_or_default(), vote_average: item.vote_average,
        poster_url: image_url(item.poster_path.as_deref(), "w780"), backdrop_url: image_url(item.backdrop_path.as_deref(), "w1280") })
}

fn choose_trailer(videos: Vec<RawVideo>) -> Option<TmdbTrailer> {
    choose_video(videos.into_iter().filter(|video| video.video_type == "Trailer").collect(), false)
}

fn choose_video(videos: Vec<RawVideo>, is_teaser: bool) -> Option<TmdbTrailer> {
    videos
        .into_iter()
        .filter(|video| {
            video.site == "YouTube"
                && video.key.len() == 11
                && video.key.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
        })
        .max_by_key(|video| (video.official, video.size))
        .map(|video| TmdbTrailer {
            key: video.key,
            name: video.name,
            is_teaser,
        })
}

fn normalize_title(title: &str) -> String {
    title
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn clean_title_query(title: &str) -> String {
    let mut words = Vec::new();
    for raw in title.split(|character: char| character.is_whitespace() || matches!(character, '.' | '_' | '-' | '[' | ']' | '(' | ')')) {
        let token = raw.trim();
        if token.is_empty() { continue; }
        let upper = token.to_ascii_uppercase();
        let episode_marker = upper.len() >= 4 && (upper.starts_with('S') && upper.contains('E') || upper.contains('X'))
            && upper.chars().any(|character| character.is_ascii_digit());
        let technical = ["480P", "720P", "1080P", "2160P", "BLURAY", "WEBRIP", "WEB-DL", "WEB_DL", "HDTV", "X264", "X265", "H264", "H265", "HEVC", "AV1", "HDR", "AAC", "DTS", "MKV", "MP4"]
            .iter().any(|marker| upper.contains(marker));
        if episode_marker || technical { break; }
        words.push(token);
    }
    let cleaned = words.join(" ");
    if cleaned.chars().count() >= 2 { cleaned } else { title.trim().to_owned() }
}

fn choose_png_logo(logos: Vec<RawLogo>) -> Option<String> {
    logos
        .into_iter()
        .filter(|logo| logo.file_path.to_ascii_lowercase().ends_with(".png"))
        .filter_map(|logo| image_url(Some(&logo.file_path), "original").map(|url| (logo, url)))
        .max_by(|(left, _), (right, _)| {
            logo_language_rank(left.iso_639_1.as_deref())
                .cmp(&logo_language_rank(right.iso_639_1.as_deref()))
                .then_with(|| left.vote_average.total_cmp(&right.vote_average))
                .then_with(|| left.width.cmp(&right.width))
        })
        .map(|(_, url)| url)
}

fn logo_language_rank(language: Option<&str>) -> u8 {
    match language {
        Some("en") => 3,
        Some("pt") => 2,
        None => 1,
        _ => 0,
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
    use super::{choose_png_logo, choose_trailer, image_url, RawLogo, RawVideo};

    #[test]
    fn selects_a_png_logo_and_ignores_other_formats() {
        let logo = |path: &str, language: Option<&str>, width| RawLogo {
            file_path: path.to_owned(),
            iso_639_1: language.map(str::to_owned),
            vote_average: 5.0,
            width,
        };
        let selected = choose_png_logo(vec![
            logo("/large.svg", Some("en"), 4000),
            logo("/portuguese.png", Some("pt"), 1200),
            logo("/english.png", Some("en"), 900),
        ]);
        assert_eq!(
            selected.as_deref(),
            Some("https://image.tmdb.org/t/p/original/english.png")
        );
        assert_eq!(
            choose_png_logo(vec![logo("/only.svg", Some("en"), 1000)]),
            None
        );
    }

    #[test]
    fn picks_an_official_youtube_trailer_and_rejects_untrusted_keys() {
        let video = |key: &str, site: &str, official: bool, size| RawVideo {
            key: key.to_owned(),
            name: "Example".to_owned(),
            site: site.to_owned(),
            video_type: "Trailer".to_owned(),
            official,
            size,
        };
        let selected = choose_trailer(vec![
            video("abcdefghijk", "YouTube", false, 2160),
            video("official123", "YouTube", true, 1080),
            video("<bad-key?>", "YouTube", true, 4320),
            video("abcdefghijk", "Other", true, 4320),
        ])
        .expect("safe trailer");
        assert_eq!(selected.key, "official123");
    }

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
