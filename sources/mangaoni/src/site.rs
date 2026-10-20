//! The MangaOni site (<https://manga-oni.com>) as pure functions: URLs and one parser per page. The pages are server-rendered
//! HTML. The page images of a chapter are listed in the page itself, base64-encoded, so no second request is needed. The
//! search and the chapter list through the site's own `POST` need a session cookie the plug-in does not have, so the search
//! is left out.

use base64::Engine;
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use url::Url;

pub type Res<T> = Result<T, String>;

pub const BASE: &str = "https://manga-oni.com";
const HOST: &str = "manga-oni.com";

// ============================ Output (wire) ============================

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Manga {
    pub url: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    /// SManga status: 0 unknown, 1 ongoing, 2 completed, 5 cancelled, 6 on hiatus.
    pub status: i32,
    pub thumbnail_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MangasPage {
    pub mangas: Vec<Manga>,
    pub has_next_page: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Chapter {
    pub url: String,
    pub name: String,
    pub date_upload: i64,
    pub chapter_number: f32,
}

// ================================ URLs =================================

/// The most visited titles of the site: one page, no paging.
pub fn top_url() -> String {
    format!("{BASE}/top")
}

/// The titles with a recent chapter: one page, no paging.
pub fn recent_url() -> String {
    format!("{BASE}/recientes")
}

/// The search: a `POST` of the query and the page's token. The site answers with JSON.
pub fn search_url() -> String {
    format!("{BASE}/buscar")
}

/// A page of the site from a stored address (`/manga/...`, `/lector/...`), refused when it is another site's.
pub fn page(path: &str) -> Res<String> {
    let url = Url::parse(BASE).and_then(|base| base.join(path)).map_err(|e| format!("invalid address {path}: {e}"))?;
    if url.host_str() != Some(HOST) {
        return Err(format!("not a MangaOni address: {path}"));
    }
    Ok(url.to_string())
}

// =============================== Parsers ===============================

fn sel(css: &str) -> Selector {
    Selector::parse(css).expect("a valid selector")
}

fn text_of(element: ElementRef) -> String {
    element.text().collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The address of a link on the site as stored (`/manga/black-clover/`).
fn path_of(href: &str) -> String {
    Url::parse(BASE).and_then(|base| base.join(href)).map(|u| u.path().to_owned()).unwrap_or_else(|_| href.to_owned())
}

/// A picture as a full address. The covers are full addresses already; a relative one is joined to the site.
fn absolute(src: &str) -> String {
    Url::parse(BASE).and_then(|base| base.join(src)).map(|u| u.to_string()).unwrap_or_else(|_| src.to_owned())
}

/// The cards of a list page: each one is a link to a title that holds a cover. A title can be linked more than once (the
/// cover and the name), so each address is kept once.
fn cards(doc: &Html) -> Vec<Manga> {
    let mut seen = Vec::new();
    let mut mangas = Vec::new();
    for link in doc.select(&sel("a[href]")) {
        let Some(href) = link.value().attr("href") else { continue };
        if !href.contains("/manga/") {
            continue;
        }
        let Some(img) = link.select(&sel("img")).next() else { continue };
        let url = path_of(href);
        if seen.contains(&url) {
            continue;
        }
        let title = img.value().attr("alt").map(str::trim).filter(|t| !t.is_empty()).map(str::to_owned).unwrap_or_else(|| text_of(link));
        if title.is_empty() {
            continue;
        }
        seen.push(url.clone());
        let thumbnail = img.value().attr("data-src").or_else(|| img.value().attr("src")).map(absolute);
        mangas.push(Manga { url, title, description: None, genre: None, status: 0, thumbnail_url: thumbnail });
    }
    mangas
}

/// The most visited titles, as cards. The site has no next page for this list.
pub fn parse_popular(html: &str) -> MangasPage {
    MangasPage { mangas: cards(&Html::parse_document(html)), has_next_page: false }
}

/// The recently updated titles, as cards. The site has no next page for this list.
pub fn parse_latest(html: &str) -> MangasPage {
    MangasPage { mangas: cards(&Html::parse_document(html)), has_next_page: false }
}

fn status_of(text: &str) -> i32 {
    let text = text.to_lowercase();
    if text.contains("en curso") || text.contains("emision") || text.contains("emisión") {
        1
    } else if text.contains("finalizado") {
        2
    } else if text.contains("pausado") {
        6
    } else if text.contains("cancelado") {
        5
    } else {
        0
    }
}

pub fn parse_details(html: &str) -> Res<Manga> {
    let doc = Html::parse_document(html);
    let title = doc.select(&sel("h1.post-title")).next().map(text_of).filter(|t| !t.is_empty()).ok_or("the manga page has no title")?;
    let genres: Vec<String> = doc.select(&sel("#categ a")).map(text_of).filter(|g| !g.is_empty()).collect();
    // the synopsis box starts with its own heading, which is not part of the text
    let description = doc.select(&sel("#sinopsis")).next().map(text_of).map(|t| t.trim_start_matches("Sinopsis").trim().to_owned()).filter(|d| !d.is_empty());
    let thumbnail = doc.select(&sel("meta[property='og:image']")).next().and_then(|m| m.value().attr("content")).map(absolute);
    Ok(Manga {
        url: String::new(),
        title,
        description,
        genre: Some(genres.join(", ")).filter(|g| !g.is_empty()),
        status: doc.select(&sel("span.estado")).next().map(|s| status_of(&text_of(s))).unwrap_or(0),
        thumbnail_url: thumbnail,
    })
}

/// The chapters of a title, newest first as the site lists them. Each one has its date (`2026-07-31 01:25:00`) and its
/// name (`Capítulo 392.5 — The title`).
pub fn parse_chapters(html: &str) -> Vec<Chapter> {
    let doc = Html::parse_document(html);
    doc.select(&sel("#c_list a[href]"))
        .map(|a| {
            let name = a.select(&sel("h3.entry-title-h2")).next().map(text_of).unwrap_or_default();
            let date_upload = a.select(&sel("span.timeago")).next().and_then(|t| t.value().attr("datetime")).and_then(date_to_unix).unwrap_or(0);
            Chapter { url: path_of(a.value().attr("href").unwrap_or_default()), chapter_number: number_in(&name), name, date_upload }
        })
        .collect()
}

/// The number after `Capítulo` (`Capítulo 392.5 — ...`); -1 when there is none.
fn number_in(name: &str) -> f32 {
    name.split_whitespace().nth(1).and_then(|word| word.parse::<f32>().ok()).unwrap_or(-1.0)
}

/// Midnight UTC of a `YYYY-MM-DD` date (the time after it is dropped), in Unix seconds.
fn date_to_unix(datetime: &str) -> Option<i64> {
    let date = datetime.split_whitespace().next()?;
    let mut parts = date.split('-').map(|p| p.parse::<i64>().ok());
    let (year, month, day) = (parts.next()??, parts.next()??, parts.next()??);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86_400)
}

/// Days from 1970-01-01 to a civil date (Howard Hinnant's algorithm).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The page images of a chapter, in order. The reader script holds `var unicap = "<base64>"`, which decodes to the image
/// folder, `||`, then a JSON list of the file names.
pub fn parse_pages(html: &str) -> Res<Vec<String>> {
    let encoded = unicap_of(html).ok_or("the chapter page has no page list")?;
    let raw = base64::engine::general_purpose::STANDARD.decode(encoded).map_err(|e| format!("invalid page list: {e}"))?;
    let raw = String::from_utf8(raw).map_err(|e| format!("invalid page list: {e}"))?;
    let mut parts = raw.split("||");
    let folder = parts.next().ok_or("the page list is empty")?;
    let names: Vec<String> = serde_json::from_str(parts.next().unwrap_or("[]")).map_err(|e| format!("invalid page names: {e}"))?;
    Ok(names.into_iter().map(|name| format!("{folder}{name}")).collect())
}

/// The quoted value of the `unicap` variable in the page's script.
fn unicap_of(html: &str) -> Option<&str> {
    let start = html.find("unicap")? + "unicap".len();
    let rest = html[start..].trim_start().strip_prefix('=')?.trim_start();
    let quote = rest.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let body = &rest[1..];
    Some(&body[..body.find(quote)?])
}

/// The token that the site's forms send back (`<meta name="csrf-token">`), from any of its pages.
pub fn csrf_token(html: &str) -> Option<String> {
    Html::parse_document(html)
        .select(&sel("meta[name='csrf-token']"))
        .next()
        .and_then(|m| m.value().attr("content"))
        .map(str::to_owned)
        .filter(|t| !t.is_empty())
}

#[derive(Deserialize)]
struct SearchReply {
    mangas: Vec<SearchItem>,
}

#[derive(Deserialize)]
struct SearchItem {
    #[serde(default)]
    nombre: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    img: String,
    /// The site marks a title it hides with 0.
    #[serde(default)]
    visible: Option<i32>,
}

/// The search reply: a JSON list of titles, each with its name, address and cover. There is no next page.
pub fn parse_search(body: &str) -> Res<MangasPage> {
    let reply: SearchReply = serde_json::from_str(body).map_err(|e| format!("invalid search reply: {e}"))?;
    let mangas = reply
        .mangas
        .into_iter()
        .filter(|m| m.visible != Some(0) && !m.nombre.trim().is_empty() && !m.url.is_empty())
        .map(|m| Manga {
            url: path_of(&m.url),
            title: m.nombre.trim().to_owned(),
            description: None,
            genre: None,
            status: 0,
            thumbnail_url: (!m.img.is_empty()).then(|| absolute(&m.img)),
        })
        .collect();
    Ok(MangasPage { mangas, has_next_page: false })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOP: &str = include_str!("../fixtures/top.html");
    const RECENT: &str = include_str!("../fixtures/recent.html");
    const MANGA: &str = include_str!("../fixtures/manga.html");
    const CHAPTER: &str = include_str!("../fixtures/chapter.html");

    #[test]
    fn the_top_list_gives_each_title_once_with_its_cover() {
        let page = parse_popular(TOP);
        assert_eq!(page.mangas.len(), 2, "the cover and the name link to one title");
        assert_eq!((page.mangas[0].url.as_str(), page.mangas[0].title.as_str()), ("/manga/titulo-uno/", "Titulo uno"));
        assert_eq!(page.mangas[0].thumbnail_url.as_deref(), Some("https://cdn.example.test/titulo-uno/cover.jpg?1"));
        assert!(!page.has_next_page);
    }

    #[test]
    fn the_recent_list_is_read_the_same_way() {
        let page = parse_latest(RECENT);
        assert_eq!(page.mangas.len(), 1);
        assert_eq!(page.mangas[0].title, "Titulo dos");
    }

    #[test]
    fn the_title_page_has_name_summary_genres_status_and_cover() {
        let manga = parse_details(MANGA).unwrap();
        assert_eq!(manga.title, "Titulo uno");
        assert_eq!(manga.status, 2);
        assert_eq!(manga.genre.as_deref(), Some("Accion, Aventura"));
        assert_eq!(manga.description.as_deref(), Some("Una sinopsis de prueba."));
        assert_eq!(manga.thumbnail_url.as_deref(), Some("https://cdn.example.test/titulo-uno/cover.jpg?1"));
    }

    #[test]
    fn the_chapters_have_numbers_and_dates() {
        let chapters = parse_chapters(MANGA);
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].url, "/lector/titulo-uno/300/");
        assert_eq!((chapters[0].chapter_number, chapters[0].date_upload), (200.0, 1_792_195_200));
        assert_eq!(chapters[1].chapter_number, 199.5);
        assert_eq!(number_in("Extra"), -1.0);
    }

    #[test]
    fn the_search_reply_gives_the_visible_titles_with_their_covers() {
        let page = parse_search(include_str!("../fixtures/search.json")).unwrap();
        assert_eq!(page.mangas.len(), 1, "the hidden title is left out");
        assert_eq!((page.mangas[0].url.as_str(), page.mangas[0].title.as_str()), ("/manga/titulo-uno/", "Titulo uno"));
        assert_eq!(page.mangas[0].thumbnail_url.as_deref(), Some("https://cdn.example.test/titulo-uno/thumb.jpg?1"));
        assert!(parse_search("not json").is_err());
    }

    #[test]
    fn the_token_comes_from_the_meta_tag() {
        assert_eq!(csrf_token(r#"<meta name="csrf-token" content="abc123">"#).as_deref(), Some("abc123"));
        assert_eq!(csrf_token("<html></html>"), None);
    }

    #[test]
    fn the_reader_gives_its_images_in_order() {
        let pages = parse_pages(CHAPTER).unwrap();
        assert_eq!(pages, vec!["https://cdn.example.test/titulo-uno/200/001.webp?up=1", "https://cdn.example.test/titulo-uno/200/002.webp?up=1"]);
    }

    #[test]
    fn only_addresses_of_the_site_are_fetched() {
        assert_eq!(page("/manga/titulo-uno/").unwrap(), "https://manga-oni.com/manga/titulo-uno/");
        assert!(page("https://evil.test/x").is_err());
        assert_eq!(date_to_unix("1970-01-02 10:00:00"), Some(86_400));
    }
}
