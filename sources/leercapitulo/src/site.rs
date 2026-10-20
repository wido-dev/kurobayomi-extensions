//! The LeerCapitulo site (<https://www.leercapitulo.co>) as pure functions: URLs and one parser per page. The pages are
//! server-rendered HTML, so no script runs. Without the filters (genre, status, type, sort).

use scraper::{ElementRef, Html, Selector};
use serde::Serialize;
use url::Url;

pub type Res<T> = Result<T, String>;

pub const BASE: &str = "https://www.leercapitulo.co";
const HOST: &str = "www.leercapitulo.co";

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

pub fn home_url() -> String {
    format!("{BASE}/")
}

/// The directory, sorted A to Z: the site has no popularity list.
pub fn directory_url(page: i32) -> String {
    format!("{BASE}/manga/?page={}", page.max(1))
}

pub fn search_url(page: i32, query: &str) -> String {
    let q: String = url::form_urlencoded::byte_serialize(query.trim().as_bytes()).collect();
    format!("{BASE}/manga/?name={q}&page={}", page.max(1))
}

/// A page of the site from a stored address (`/manga/...`, `/leer/...`), refused when it is another site's.
pub fn page(path: &str) -> Res<String> {
    let url = Url::parse(BASE).and_then(|base| base.join(path)).map_err(|e| format!("invalid address {path}: {e}"))?;
    if url.host_str() != Some(HOST) {
        return Err(format!("not a LeerCapitulo address: {path}"));
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

/// The address of a link on the site as stored (`/manga/psvkfbmjgo/one-piece/`).
fn path_of(href: &str) -> String {
    Url::parse(BASE).and_then(|base| base.join(href)).map(|u| u.path().to_owned()).unwrap_or_else(|_| href.to_owned())
}

/// A picture on the site as a full address: the covers are relative (`/covers/...`).
fn absolute(src: &str) -> String {
    Url::parse(BASE).and_then(|base| base.join(src)).map(|u| u.to_string()).unwrap_or_else(|_| src.to_owned())
}

/// The `page` number of an address with a `page=` query, such as the next link of the directory.
fn page_number(href: &str) -> Option<i32> {
    Url::parse(BASE).and_then(|base| base.join(href)).ok()?.query_pairs().find(|(key, _)| key == "page")?.1.parse().ok()
}

/// A manga card of the directory or of a search: a cover, the title link and the latest chapter link.
fn card(element: ElementRef) -> Option<Manga> {
    let link = element.select(&sel("a.lc-card-name")).next()?;
    let title = text_of(link);
    if title.is_empty() {
        return None;
    }
    Some(Manga {
        url: path_of(link.value().attr("href")?),
        title,
        description: None,
        genre: None,
        status: 0,
        thumbnail_url: element.select(&sel("img")).next().and_then(|i| i.value().attr("src")).map(absolute),
    })
}

/// The cards of a page of the directory or of a search. There is a next page when a link names the page after this one.
fn listing(html: &str, page: i32) -> MangasPage {
    let doc = Html::parse_document(html);
    let mangas = doc.select(&sel("article.lc-card")).filter_map(card).collect();
    let has_next_page = doc.select(&sel("a[href]")).filter_map(|a| page_number(a.value().attr("href")?)).any(|n| n == page + 1);
    MangasPage { mangas, has_next_page }
}

pub fn parse_popular(html: &str, page: i32) -> MangasPage {
    listing(html, page)
}

pub fn parse_search(html: &str, page: i32) -> MangasPage {
    listing(html, page)
}

/// The latest updates of the home page: the slider lists one manga per slide, with its latest chapter.
pub fn parse_latest(html: &str) -> MangasPage {
    let doc = Html::parse_document(html);
    let covers = doc.select(&sel("a.lc-slide-cover")).collect::<Vec<_>>();
    let mangas = doc
        .select(&sel("a.lc-slide-name"))
        .filter_map(|name| {
            let href = name.value().attr("href")?;
            let thumbnail = covers
                .iter()
                .find(|cover| cover.value().attr("href") == Some(href))
                .and_then(|cover| cover.select(&sel("img")).next())
                .and_then(|i| i.value().attr("src"))
                .map(absolute);
            Some(Manga { url: path_of(href), title: text_of(name), description: None, genre: None, status: 0, thumbnail_url: thumbnail })
        })
        .collect();
    MangasPage { mangas, has_next_page: false }
}

fn status_of(text: &str) -> i32 {
    let text = text.to_lowercase();
    if text.contains("ongoing") {
        1
    } else if text.contains("completed") {
        2
    } else if text.contains("hiatus") {
        6
    } else if text.contains("cancel") {
        5
    } else {
        0
    }
}

/// The value of a fact of the manga page (`<li><span class="k">Estado</span><a>Ongoing</a></li>`).
fn fact(doc: &Html, label: &str) -> Option<String> {
    doc.select(&sel(".lc-facts li"))
        .find(|li| li.select(&sel("span.k")).next().map(text_of).as_deref() == Some(label))
        .and_then(|li| li.select(&sel("a")).next())
        .map(text_of)
}

pub fn parse_details(html: &str) -> Res<Manga> {
    let doc = Html::parse_document(html);
    let title = doc.select(&sel("h1")).next().map(text_of).filter(|t| !t.is_empty()).ok_or("the manga page has no title")?;
    let genres: Vec<String> = doc.select(&sel("a[href*='genre=']")).map(text_of).collect();
    Ok(Manga {
        url: String::new(),
        title,
        description: doc.select(&sel("#sinopsis p")).next().map(text_of).filter(|d| !d.is_empty()),
        genre: Some(genres.join(", ")).filter(|g| !g.is_empty()),
        status: fact(&doc, "Estado").map(|s| status_of(&s)).unwrap_or(0),
        thumbnail_url: doc.select(&sel(".lc-cover-lg img")).next().and_then(|i| i.value().attr("src")).map(absolute),
    })
}

/// The chapters of a manga page, newest first as the site lists them, each with its date (`2026-10-17`).
pub fn parse_chapters(html: &str) -> Vec<Chapter> {
    let doc = Html::parse_document(html);
    doc.select(&sel("#chapterList a.lc-chapter-row"))
        .map(|a| {
            let name = a.select(&sel("span.n")).next().map(text_of).unwrap_or_default();
            let date_upload = a.select(&sel("span.d")).next().map(text_of).and_then(|d| date_to_unix(&d)).unwrap_or(0);
            Chapter { url: path_of(a.value().attr("href").unwrap_or_default()), chapter_number: number_in(&name), name, date_upload }
        })
        .collect()
}

/// The number in a chapter name (`Capitulo 1195`, `Capitulo 20.01`); -1 when there is none.
fn number_in(name: &str) -> f32 {
    name.split_whitespace().rev().find_map(|word| word.parse::<f32>().ok()).unwrap_or(-1.0)
}

/// Midnight UTC of a `YYYY-MM-DD` date, in Unix seconds.
fn date_to_unix(date: &str) -> Option<i64> {
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

/// The page images of a chapter, in order. The reader lazy-loads them, so the address is in `data-src` at first.
pub fn parse_pages(html: &str) -> Vec<String> {
    Html::parse_document(html)
        .select(&sel("#lcPages img"))
        .filter_map(|img| img.value().attr("data-src").or_else(|| img.value().attr("src")))
        .map(absolute)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = include_str!("../fixtures/home.html");
    const DIRECTORY: &str = include_str!("../fixtures/directory.html");
    const SEARCH: &str = include_str!("../fixtures/search.html");
    const MANGA: &str = include_str!("../fixtures/manga.html");
    const CHAPTER: &str = include_str!("../fixtures/chapter.html");

    #[test]
    fn the_home_page_gives_the_latest_updates_with_covers() {
        let page = parse_latest(HOME);
        assert_eq!(page.mangas.len(), 2);
        assert_eq!((page.mangas[0].url.as_str(), page.mangas[0].title.as_str()), ("/manga/aaa111/manga-de-prueba/", "Manga de prueba"));
        assert_eq!(page.mangas[0].thumbnail_url.as_deref(), Some("https://www.leercapitulo.co/covers/aa/1.jpg?v1"));
        assert!(!page.has_next_page);
    }

    #[test]
    fn the_directory_lists_cards_and_a_next_page_link_means_more() {
        let page = parse_popular(DIRECTORY, 1);
        assert_eq!(page.mangas.len(), 2);
        assert_eq!(page.mangas[1].title, "Otro manga de prueba");
        assert!(page.has_next_page);
        assert!(!parse_popular(DIRECTORY, 2).has_next_page, "page 2 has no link to page 3 in the fixture");
        assert_eq!(directory_url(0), "https://www.leercapitulo.co/manga/?page=1");
    }

    #[test]
    fn a_search_lists_cards_and_builds_the_query_address() {
        let page = parse_search(SEARCH, 1);
        assert_eq!(page.mangas[0].url, "/manga/bbb222/busqueda-de-prueba/");
        assert!(page.has_next_page);
        assert_eq!(search_url(2, "one piece"), "https://www.leercapitulo.co/manga/?name=one+piece&page=2");
    }

    #[test]
    fn the_manga_page_has_title_summary_genres_status_and_cover() {
        let manga = parse_details(MANGA).unwrap();
        assert_eq!(manga.title, "Manga de prueba");
        assert_eq!(manga.status, 1);
        assert_eq!(manga.genre.as_deref(), Some("Action, Adventure"));
        assert_eq!(manga.description.as_deref(), Some("Una sinopsis corta de prueba."));
        assert_eq!(manga.thumbnail_url.as_deref(), Some("https://www.leercapitulo.co/covers/aa/1.jpg?v1"));
    }

    #[test]
    fn the_chapters_have_numbers_and_dates() {
        let chapters = parse_chapters(MANGA);
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].name, "Capitulo 2");
        assert_eq!(chapters[0].url, "/leer/aaa111/manga-de-prueba/2/");
        assert_eq!((chapters[0].chapter_number, chapters[0].date_upload), (2.0, 1_792_195_200));
        assert_eq!(chapters[1].chapter_number, 20.01);
        assert_eq!(number_in("Extra"), -1.0);
    }

    #[test]
    fn the_reader_gives_its_images_in_order() {
        let pages = parse_pages(CHAPTER);
        assert_eq!(pages.len(), 2);
        assert!(pages[0].starts_with("https://") && pages[0] != pages[1]);
    }

    #[test]
    fn dates_become_unix_seconds_and_only_the_site_is_fetched() {
        assert_eq!(date_to_unix("1970-01-02"), Some(86_400));
        assert_eq!(date_to_unix("2026-13-01"), None);
        assert_eq!(page("/manga/aaa111/manga-de-prueba/").unwrap(), "https://www.leercapitulo.co/manga/aaa111/manga-de-prueba/");
        assert!(page("https://evil.test/x").is_err());
    }
}
