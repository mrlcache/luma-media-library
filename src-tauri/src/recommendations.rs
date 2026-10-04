use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};
use media_core::{LibraryState, LibraryStore};
use serde::Serialize;
use crate::tmdb::{media_endpoint, TmdbSearchResult, TmdbState, TmdbTrailer};

#[derive(Clone, Serialize)]
pub struct RecommendationSection { title: String, items: Vec<TmdbSearchResult> }
#[derive(Clone, Serialize)]
pub struct DiscoveryFeed { featured: Vec<TmdbSearchResult>, sections: Vec<RecommendationSection>, warning: Option<String> }

fn key(item: &TmdbSearchResult) -> (String, u64) { (item.kind.clone(), item.id) }

// Weighted reciprocal rank fusion: recent viewing is stronger than ownership;
// repeated recommendations from different seeds reinforce the same candidate.
fn fuse(lists: &[(f64, Vec<TmdbSearchResult>)], owned: &HashSet<(String, u64)>) -> Vec<TmdbSearchResult> {
    let mut candidates: HashMap<(String, u64), (TmdbSearchResult, f64)> = HashMap::new();
    for (weight, items) in lists {
        let mut seen = HashSet::new();
        for (rank, item) in items.iter().enumerate() {
            let id = key(item);
            if owned.contains(&id) || item.poster_url.is_none() || !seen.insert(id.clone()) { continue; }
            let candidate = candidates.entry(id).or_insert_with(|| (item.clone(), 0.0));
            candidate.1 += weight / (20.0 + rank as f64);
        }
    }
    let mut candidates: Vec<_> = candidates.into_values().collect();
    candidates.sort_by(|a,b| b.1.total_cmp(&a.1).then_with(|| a.0.kind.cmp(&b.0.kind)).then_with(|| a.0.id.cmp(&b.0.id)));
    candidates.into_iter().map(|(item,_)| item).collect()
}

#[tauri::command]
pub async fn get_discovery_feed(library: tauri::State<'_, LibraryState>, tmdb: tauri::State<'_, TmdbState>) -> Result<DiscoveryFeed, String> {
    let db_path = library.db_path.clone();
    let profile = tauri::async_runtime::spawn_blocking(move || LibraryStore::open(&db_path)?.recommendation_profile())
        .await.map_err(|_| "Could not read your recommendation profile.".to_owned())??;
    let result = build_feed(profile, tmdb.inner()).await;
    if let Err(error) = &result { eprintln!("Discovery feed: {error}"); }
    #[cfg(debug_assertions)]
    if let Ok(feed) = &result { eprintln!("Discovery feed ready: {} featured titles, {} sections", feed.featured.len(), feed.sections.len()); }
    result
}

async fn build_feed(profile: Vec<(u64, String, String, Option<i64>)>, tmdb: &TmdbState) -> Result<DiscoveryFeed, String> {
    let owned: HashSet<_> = profile.iter().map(|(id,kind,_,_)| (kind.clone(),*id)).collect();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as i64;
    let mut requests = Vec::new();
    // Include recent acquisitions as well as viewing, even with a long watch history.
    let mut seeds: Vec<_> = profile.iter().filter(|(_,_,_,watched)| watched.is_some()).take(4).cloned().collect();
    seeds.extend(profile.iter().filter(|(_,_,_,watched)| watched.is_none()).take(2).cloned());
    let mut selected: HashSet<_> = seeds.iter().map(|(id,kind,_,_)| (kind.clone(),*id)).collect();
    for seed in &profile {
        if seeds.len() >= 6 { break; }
        if selected.insert((seed.1.clone(),seed.0)) { seeds.push(seed.clone()); }
    }
    for (id,kind,title,watched) in seeds {
        let client = tmdb.clone();
        let weight = watched.map(|time| 1.0 + 4.0 * 2_f64.powf(-((now-time).max(0) as f64)/(30.0*86400000.0))).unwrap_or(1.0);
        requests.push((title, weight, tauri::async_runtime::spawn(async move {
            let endpoint = media_endpoint(&kind)?;
            client.discovery_list(&format!("{endpoint}/{id}/recommendations"), Some(&kind)).await
        })));
    }
    let movie_client = tmdb.clone();
    let series_client = tmdb.clone();
    let movies = tauri::async_runtime::spawn(async move { movie_client.discovery_list("trending/movie/week", Some("movie")).await });
    let series = tauri::async_runtime::spawn(async move { series_client.discovery_list("trending/tv/week", Some("series")).await });
    let mut lists = Vec::new();
    let mut because = None;
    let mut failed = false;
    for (title,weight,request) in requests {
        match request.await {
            Ok(Ok(items)) => { if because.is_none() && !items.is_empty() { because = Some((title,items.clone())); } lists.push((weight,items)); },
            _ => failed = true,
        }
    }
    let movies = movies.await.map_err(|_| "Could not load movie recommendations.".to_owned())?;
    let series = series.await.map_err(|_| "Could not load series recommendations.".to_owned())?;
    if lists.iter().all(|(_,items)| items.is_empty()) && movies.is_err() && series.is_err() { return Err("Recommendations are unavailable. Check your TMDb connection in Settings.".to_owned()); }
    failed |= movies.is_err() || series.is_err();
    let movies = movies.unwrap_or_default();
    let series = series.unwrap_or_default();
    lists.push((0.35,movies.clone())); lists.push((0.35,series.clone()));
    let ranked = fuse(&lists,&owned);
    let featured = ranked.iter().filter(|item| item.backdrop_url.is_some()).take(8).cloned().collect();
    let mut sections = Vec::new();
    let mut shown = HashSet::new();
    let mut section = |title: String, items: Vec<TmdbSearchResult>| {
        let items: Vec<_> = items.into_iter().filter(|item| !owned.contains(&key(item)) && item.poster_url.is_some() && shown.insert(key(item))).take(12).collect();
        if !items.is_empty() { sections.push(RecommendationSection {title,items}); }
    };
    section("For you".to_owned(),ranked.into_iter().take(12).collect());
    if let Some((title,items)) = because { section(format!("Because you like {title}"),items); }
    section("Movies to discover".to_owned(),movies);
    section("Series to discover".to_owned(),series);
    Ok(DiscoveryFeed {featured,sections,warning:failed.then(|| "Some recommendations could not be refreshed.".to_owned())})
}

#[tauri::command]
pub async fn get_discovery_title(id: u64, kind: String, tmdb: tauri::State<'_, TmdbState>) -> Result<TmdbSearchResult, String> { tmdb.discovery_title(id,&kind).await }
#[tauri::command]
pub async fn get_discovery_trailer(id: u64, kind: String, tmdb: tauri::State<'_, TmdbState>) -> Result<Option<TmdbTrailer>, String> { media_endpoint(&kind)?; tmdb.trailer_for_kind(id,&kind).await }
#[tauri::command]
pub async fn get_discovery_logo(id: u64, kind: String, tmdb: tauri::State<'_, TmdbState>) -> Result<Option<String>, String> { media_endpoint(&kind)?; tmdb.logo_for_kind(id,&kind).await }

#[cfg(test)]
mod tests {
    use super::*;
    fn item(id:u64,kind:&str)->TmdbSearchResult { TmdbSearchResult {id,title:format!("Title {id}"),kind:kind.into(),year:Some(2024),overview:String::new(),vote_average:Some(7.0),poster_url:Some("https://image.tmdb.org/t/p/w780/example.jpg".into()),backdrop_url:None} }
    #[test]
    fn fusion_uses_history_weights_and_combines_independent_seeds() {
        let ranked=fuse(&[(1.0,vec![item(1,"movie"),item(2,"movie")]),(5.0,vec![item(3,"movie"),item(2,"movie")])],&HashSet::new());
        assert_eq!(ranked[0].id,2); assert_eq!(ranked[1].id,3);
    }
    #[test]
    fn ownership_is_kind_scoped_and_duplicate_candidates_do_not_inflate_score() {
        let owned=HashSet::from([("movie".into(),1)]);
        let ranked=fuse(&[(1.0,vec![item(1,"movie"),item(1,"series"),item(2,"movie"),item(2,"movie")])],&owned);
        assert_eq!(ranked.len(),2); assert_eq!(ranked[0].kind,"series");
    }
    #[cfg(windows)]
    #[test]
    #[ignore = "requires the existing local library, TMDb credential and network"]
    fn live_library_can_generate_a_complete_discovery_feed() {
        let app_data=std::path::PathBuf::from(std::env::var_os("APPDATA").expect("APPDATA")).join("local.media.platform");
        let profile=LibraryStore::open(&app_data.join("media-library.sqlite3")).unwrap().recommendation_profile().unwrap();
        let owned:HashSet<_>=profile.iter().map(|(id,kind,_,_)|(kind.clone(),*id)).collect();
        let tmdb=TmdbState::new(app_data.join("tmdb_read_access_token")).unwrap();
        let feed=tauri::async_runtime::block_on(build_feed(profile,&tmdb)).expect("real discovery feed");
        assert!(!feed.featured.is_empty()); assert!(!feed.sections.is_empty());
        assert!(feed.featured.iter().chain(feed.sections.iter().flat_map(|section|&section.items)).all(|item|!owned.contains(&key(item))));
        println!("Verified {} featured titles and {} recommendation sections.",feed.featured.len(),feed.sections.len());
    }
}
