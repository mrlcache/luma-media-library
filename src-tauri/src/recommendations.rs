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

type Seed = (u64, String, String, Option<i64>);

fn recommendation_seeds(profile: &[Seed], cycle: u64) -> Vec<Seed> {
    let mut seeds: Vec<_> = profile.iter().filter(|seed| seed.3.is_some()).take(3).cloned().collect();
    seeds.extend(profile.iter().filter(|seed| seed.3.is_none()).take(1).cloned());
    let mut selected: HashSet<_> = seeds.iter().map(|seed| (seed.1.clone(),seed.0)).collect();
    let remaining: Vec<_> = profile.iter().filter(|seed| !selected.contains(&(seed.1.clone(),seed.0))).collect();
    if !remaining.is_empty() {
        let offset = cycle as usize % remaining.len();
        for seed in remaining.iter().cycle().skip(offset).take(remaining.len()) {
            if seeds.len() >= 6 { break; }
            if selected.insert((seed.1.clone(),seed.0)) { seeds.push((*seed).clone()); }
        }
    }
    seeds
}

// Keep strong matches while giving relevant candidates beyond the first page exposure.
fn diversify(ranked: Vec<TmdbSearchResult>, cycle: u64) -> Vec<TmdbSearchResult> {
    if ranked.len() <= 4 { return ranked; }
    let anchors = &ranked[..4];
    let pool = &ranked[4..ranked.len().min(36)];
    let mut result = Vec::with_capacity(ranked.len());
    let mut seen = HashSet::new();
    let offset = (cycle as usize % pool.len()) * 8 % pool.len();
    for index in 0..12 {
        let item = if index < 8 && index % 2 == 0 { &anchors[(index/2 + cycle as usize % 4) % 4] }
            else { &pool[(offset + if index < 8 { index/2 } else { index-4 }) % pool.len()] };
        if seen.insert(key(item)) { result.push(item.clone()); }
    }
    for item in ranked { if seen.insert(key(&item)) { result.push(item); } }
    result
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
    let cycle = (now.max(0) / (5 * 60 * 1000)) as u64;
    let mut requests = Vec::new();
    // Include recent acquisitions as well as viewing, even with a long watch history.
    let seeds = recommendation_seeds(&profile, cycle);
    for (seed_index, (id,kind,title,watched)) in seeds.into_iter().enumerate() {
        let client = tmdb.clone();
        let weight = watched.map(|time| 1.0 + 6.0 * 2_f64.powf(-((now-time).max(0) as f64)/(7.0*86400000.0))).unwrap_or(0.6);
        requests.push((title, weight, tauri::async_runtime::spawn(async move {
            let endpoint = media_endpoint(&kind)?;
            let endpoint = format!("{endpoint}/{id}/recommendations");
            if seed_index < 2 {
                let (first, second) = tokio::join!(client.discovery_list_page(&endpoint, Some(&kind), 1), client.discovery_list_page(&endpoint, Some(&kind), 2));
                let mut items = first?;
                if let Ok(extra) = second { items.extend(extra); }
                Ok(items)
            } else { client.discovery_list(&endpoint, Some(&kind)).await }
        })));
    }
    let movie_client = tmdb.clone();
    let series_client = tmdb.clone();
    let movies = tauri::async_runtime::spawn(async move { movie_client.discovery_list("trending/movie/day", Some("movie")).await });
    let series = tauri::async_runtime::spawn(async move { series_client.discovery_list("trending/tv/day", Some("series")).await });
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
    let ranked = diversify(fuse(&lists,&owned), cycle);
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
    #[test]
    fn refresh_changes_exploration_without_losing_strong_matches_or_titles() {
        let ranked: Vec<_> = (1..=45).map(|id| item(id,"movie")).collect();
        let first=diversify(ranked.clone(),0); let next=diversify(ranked.clone(),1);
        assert_eq!(first.iter().map(key).collect::<HashSet<_>>(), ranked.iter().map(key).collect());
        assert_eq!(first.len(),ranked.len());
        assert_ne!(first[..12].iter().map(key).collect::<HashSet<_>>(),next[..12].iter().map(key).collect());
        for id in 1..=4 { assert!(next[..8].iter().any(|item|item.id==id)); }
        assert_eq!(next.iter().map(key).collect::<Vec<_>>(),diversify(ranked,1).iter().map(key).collect::<Vec<_>>());
    }
    #[test]
    fn older_seeds_rotate_while_recent_watching_stays_relevant() {
        let profile: Vec<_> = (1..=10).map(|id|(id,"movie".into(),format!("Title {id}"),Some(1))).collect();
        let first=recommendation_seeds(&profile,0); let next=recommendation_seeds(&profile,1);
        assert_eq!(next.len(),6);
        assert_eq!(next[..3],first[..3]);
        assert_ne!(next[3..],first[3..]);
        assert_eq!(next.iter().map(|seed|seed.0).collect::<HashSet<_>>().len(),6);
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
