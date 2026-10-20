//! The JKAnime site (<https://jkanime.net>) as pure functions: URLs and one parser per page. The pages are server-rendered
//! HTML. The episode list of a title is not in its page (the site fills it with a `POST`, which needs a session the plug-in
//! does not hold), so the episodes are counted from the page and the `videos` come from the site's own player pages.

use scraper::{ElementRef, Html, Selector};
use serde::Serialize;
use url::Url;

pub type Res<T> = Result<T, String>;

pub const BASE: &str = "https://jkanime.net";
const HOST: &str = "jkanime.net";

/// The site's own players: `um` and `umv` answer with an HLS playlist; the others are not read.
const PLAYERS: [&str; 2] = ["um", "umv"];

// ============================ Output (wire) ============================

#[derive(Debug, Clone, PartialEq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Anime {
    pub url: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub thumbnail_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    /// SAnime status: 0 unknown, 1 ongoing, 2 completed.
    #[serde(default)]
    pub status: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimesPage {
    pub animes: Vec<Anime>,
    pub has_next_page: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Episode {
    pub url: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub episode_number: f32,
    #[serde(default)]
    pub scanlator: String,
}

/// One player of an episode: its page, read later by the `videos` op.
#[derive(Debug, Clone, PartialEq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hoster {
    pub hoster_url: String,
    pub hoster_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Video {
    pub video_url: String,
    pub video_title: String,
    pub resolution: Option<i32>,
    pub headers: Vec<(String, String)>,
    pub preferred: bool,
}

// ================================ URLs =================================

/// The headers a video request needs: the site as referer.
pub fn video_headers() -> Vec<(String, String)> {
    vec![("Referer".to_owned(), format!("{BASE}/"))]
}

/// The search page for a query. The site's own search is a path (`/buscar/<text>/`), and it has no next page.
pub fn search_url(query: &str) -> String {
    let text: String = url::form_urlencoded::byte_serialize(query.trim().as_bytes()).collect::<String>().replace('+', "%20");
    format!("{BASE}/buscar/{text}/")
}

/// The home page: the hero of the featured titles. Read for the popular list, which has no paging.
pub fn home_url() -> String {
    format!("{BASE}/")
}

/// A page of the site from a stored address (`/slug/`, `/slug/3`), refused when it is another site's.
pub fn page(path: &str) -> Res<String> {
    let url = Url::parse(BASE).and_then(|base| base.join(path)).map_err(|e| format!("invalid address {path}: {e}"))?;
    if url.host_str() != Some(HOST) {
        return Err(format!("not a JKAnime address: {path}"));
    }
    Ok(url.to_string())
}

/// The path of an address on the site (`/one-piece/`).
fn path_of(href: &str) -> String {
    Url::parse(BASE).and_then(|base| base.join(href)).map(|u| u.path().to_owned()).unwrap_or_else(|_| href.to_owned())
}

/// The address of a page or a picture on the site, as a full address.
pub fn absolute(href: &str) -> String {
    Url::parse(BASE).and_then(|base| base.join(href)).map(|u| u.to_string()).unwrap_or_else(|_| href.to_owned())
}

// =============================== Parsers ===============================

fn sel(css: &str) -> Selector {
    Selector::parse(css).expect("a valid selector")
}

fn text_of(element: ElementRef) -> String {
    element.text().collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ")
}

fn meta(doc: &Html, css: &str) -> Option<String> {
    doc.select(&sel(css)).next().and_then(|m| m.value().attr("content")).map(str::to_owned).filter(|c| !c.is_empty())
}

/// The cards of the search page: a title link, its name and its picture (in `data-setbg`).
pub fn parse_search(html: &str) -> AnimesPage {
    let doc = Html::parse_document(html);
    let animes = doc
        .select(&sel(".anime__item"))
        .filter_map(|card| {
            let link = card.select(&sel("h5 a")).next()?;
            let title = text_of(link);
            if title.is_empty() {
                return None;
            }
            Some(Anime {
                url: path_of(link.value().attr("href")?),
                title,
                description: None,
                thumbnail_url: card.select(&sel("[data-setbg]")).next().and_then(|i| i.value().attr("data-setbg")).map(absolute),
                genre: None,
                status: 0,
            })
        })
        .collect();
    AnimesPage { animes, has_next_page: false }
}

/// The featured titles of the home page: each hero slide links to its title and has its picture in `data-setbg`.
pub fn parse_popular(html: &str) -> AnimesPage {
    let doc = Html::parse_document(html);
    let mut animes: Vec<Anime> = Vec::new();
    for link in doc.select(&sel("a[href]")) {
        let Some(href) = link.value().attr("href") else { continue };
        let path = path_of(href);
        // a title is one path segment: `/one-piece/`; the site's other pages have more of them
        if path.matches('/').count() != 2 || path.starts_with("/dash") || animes.iter().any(|a| a.url == path) {
            continue;
        }
        let title = text_of(link);
        if title.is_empty() {
            continue;
        }
        animes.push(Anime {
            url: path,
            title,
            description: None,
            thumbnail_url: link.select(&sel("[data-setbg]")).next().and_then(|i| i.value().attr("data-setbg")).map(absolute),
            genre: None,
            status: 0,
        });
    }
    AnimesPage { animes, has_next_page: false }
}

fn status_of(text: &str) -> i32 {
    let text = text.to_lowercase();
    if text.contains("emision") || text.contains("emisión") {
        1
    } else if text.contains("finalizado") {
        2
    } else {
        0
    }
}

/// The title page: the name (the page's own title, without the site's suffix), the summary, the genres, the status and the
/// cover.
pub fn parse_details(html: &str, url: &str) -> Res<Anime> {
    let doc = Html::parse_document(html);
    let title = meta(&doc, "meta[property='og:title']")
        .map(|t| t.split(" - anime ").next().unwrap_or_default().trim().to_owned())
        .filter(|t| !t.is_empty())
        .ok_or("the anime page has no title")?;
    let genres: Vec<String> = doc.select(&sel("a[href*='/genero/']")).map(text_of).filter(|g| !g.is_empty()).collect();
    Ok(Anime {
        url: url.to_owned(),
        title,
        description: meta(&doc, "meta[name='description']"),
        thumbnail_url: meta(&doc, "meta[property='og:image']"),
        genre: Some(genres.join(", ")).filter(|g| !g.is_empty()),
        status: doc.select(&sel(".enemision")).next().map(|s| status_of(&text_of(s))).unwrap_or(0),
    })
}

/// The number of episodes the title page gives (`<li><span>Episodios:</span> 12</li>`); 0 when it gives none.
pub fn episode_count(html: &str) -> u32 {
    let doc = Html::parse_document(html);
    doc.select(&sel(".anime__details__widget li, li"))
        .find(|li| text_of(*li).starts_with("Episodios:"))
        .and_then(|li| text_of(li).rsplit(' ').next().and_then(|n| n.parse().ok()))
        .unwrap_or(0)
}

/// The episode list of a title, from its address and the episode count: `/slug/1` to `/slug/N`.
pub fn episodes(anime_url: &str, count: u32) -> Vec<Episode> {
    let base = anime_url.trim_end_matches('/');
    (1..=count)
        .map(|n| Episode { url: format!("{base}/{n}"), name: format!("Episodio {n}"), episode_number: n as f32, scanlator: String::new() })
        .collect()
}

/// The players of an episode page that the plug-in reads (see `PLAYERS`), in the page's order.
pub fn parse_hosters(html: &str) -> Vec<Hoster> {
    Html::parse_document(html)
        .select(&sel("iframe[src*='/jkplayer/']"))
        .filter_map(|frame| {
            let src = absolute(frame.value().attr("src")?);
            let name = Url::parse(&src).ok()?.path().rsplit('/').next()?.to_owned();
            PLAYERS.contains(&name.as_str()).then(|| Hoster { hoster_url: src, hoster_name: format!("JKAnime {name}") })
        })
        .collect()
}

/// The first HLS playlist address on a player page. The player holds it as a plain string, sometimes with `\/` escapes.
pub fn parse_playlist(html: &str) -> Option<String> {
    let text = html.replace("\\/", "/");
    text.match_indices("https://").find_map(|(start, _)| {
        let rest = &text[start..];
        let end = rest.find(|c: char| c == '"' || c == '\'' || c == '\\' || c == '<' || c.is_whitespace()).unwrap_or(rest.len());
        let candidate = &rest[..end];
        candidate.contains(".m3u8").then(|| candidate.to_owned())
    })
}

/// The number the site uses for a title in its episode search (`/ajax/search_episode/<id>/<n>`), read from the title page.
pub fn anime_id(html: &str) -> Option<String> {
    let start = html.find("search_episode/")? + "search_episode/".len();
    let digits: String = html[start..].chars().take_while(|c| c.is_ascii_digit()).collect();
    (!digits.is_empty()).then_some(digits)
}

/// The token the site's forms send back (`<meta name="csrf-token">`), from its pages.
pub fn csrf_token(html: &str) -> Option<String> {
    Html::parse_document(html)
        .select(&sel("meta[name='csrf-token']"))
        .next()
        .and_then(|m| m.value().attr("content"))
        .map(str::to_owned)
        .filter(|t| !t.is_empty())
}

/// The episode search answers with a JSON list holding the episode it asked for, or an empty list past the last one.
pub fn episode_found(body: &str) -> Res<bool> {
    let items: Vec<serde_json::Value> = serde_json::from_str(body).map_err(|e| format!("invalid episode reply: {e}"))?;
    Ok(!items.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEARCH: &str = include_str!("../fixtures/search.html");
    const HOME: &str = include_str!("../fixtures/home.html");
    const ANIME: &str = include_str!("../fixtures/anime.html");
    const EPISODE: &str = include_str!("../fixtures/episode.html");
    const PLAYER: &str = include_str!("../fixtures/player.html");

    #[test]
    fn the_search_cards_give_titles_links_and_pictures() {
        let page = parse_search(SEARCH);
        assert_eq!(page.animes.len(), 2);
        assert_eq!((page.animes[0].url.as_str(), page.animes[0].title.as_str()), ("/titulo-de-prueba/", "Titulo de prueba"));
        assert_eq!(page.animes[0].thumbnail_url.as_deref(), Some("https://cdn.example.test/titulo-de-prueba.jpg"));
        assert!(!page.has_next_page);
        assert_eq!(search_url("one piece"), "https://jkanime.net/buscar/one%20piece/");
    }

    #[test]
    fn the_home_page_gives_each_featured_title_once() {
        let page = parse_popular(HOME);
        assert_eq!(page.animes.len(), 2);
        assert_eq!(page.animes[1].title, "Otro titulo");
    }

    #[test]
    fn the_title_page_has_name_summary_genres_status_and_cover() {
        let anime = parse_details(ANIME, "https://jkanime.net/titulo-de-prueba/").unwrap();
        assert_eq!(anime.title, "Titulo de prueba");
        assert_eq!(anime.status, 1);
        assert_eq!(anime.genre.as_deref(), Some("Accion, Aventura"));
        assert_eq!(anime.description.as_deref(), Some("Una descripcion de prueba."));
        assert_eq!(anime.thumbnail_url.as_deref(), Some("https://cdn.example.test/titulo-de-prueba.jpg"));
    }

    #[test]
    fn the_episode_count_and_the_episode_list_follow_the_title() {
        assert_eq!(episode_count(ANIME), 3);
        let list = episodes("/titulo-de-prueba/", 3);
        assert_eq!(list.len(), 3);
        assert_eq!((list[2].url.as_str(), list[2].name.as_str(), list[2].episode_number), ("/titulo-de-prueba/3", "Episodio 3", 3.0));
        assert!(episodes("/titulo-de-prueba/", 0).is_empty());
    }

    #[test]
    fn the_episode_page_gives_only_the_site_players() {
        let hosters = parse_hosters(EPISODE);
        let names: Vec<&str> = hosters.iter().map(|h| h.hoster_name.as_str()).collect();
        assert_eq!(names, vec!["JKAnime um", "JKAnime umv"]);
        assert!(hosters[0].hoster_url.starts_with("https://jkanime.net/jkplayer/um?e="));
    }

    #[test]
    fn the_player_gives_its_playlist() {
        assert_eq!(parse_playlist(PLAYER).as_deref(), Some("https://stream.example.test/playlist.m3u8?t=1"));
        assert_eq!(parse_playlist("<html>no stream</html>"), None);
    }

    #[test]
    fn the_episode_search_needs_the_title_id_and_the_token() {
        assert_eq!(anime_id(r#"<script>url: "https://jkanime.net/ajax/search_episode/4961/" + num</script>"#).as_deref(), Some("4961"));
        assert_eq!(anime_id("<html></html>"), None);
        assert_eq!(csrf_token(r#"<meta name="csrf-token" content="tok9">"#).as_deref(), Some("tok9"));
        assert!(!episode_found("[]").unwrap());
        assert!(episode_found(r#"[{"id":1,"number":1,"title":"x","image":"y"}]"#).unwrap());
        assert!(episode_found("oops").is_err());
    }

    #[test]
    fn only_addresses_of_the_site_are_fetched() {
        assert_eq!(page("/titulo-de-prueba/").unwrap(), "https://jkanime.net/titulo-de-prueba/");
        assert!(page("https://evil.test/x").is_err());
    }
}
