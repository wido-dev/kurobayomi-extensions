//! MangaOni (<https://manga-oni.com>), a Spanish manga source plug-in for Kurobayomi, ABI 1.5
//! (`wit/kuro-source.wit`). This file is the ABI glue: the ops and the one network path, the `http` import. The URLs and
//! parsers are in `site.rs`.

mod site;

use serde::Deserialize;
use serde_json::{Value, json};

wit_bindgen::generate!({
    path: "wit",
    world: "source",
});

use kuro::source::host;
use site::Res;

/// The source id the app derives for it: `generate_id("MangaOni", "es", 1)`, the MD5 of `mangaoni/es/1`. Keep it:
/// backups and the library name it.
const ID: i64 = 2342274476374431513;
const NAME: &str = "MangaOni";
const LANG: &str = "es";
/// One request a second keeps well under any limit the site may have.
const REQUEST_GAP_MS: u64 = 1000;

/// The `args` of an op. Each op reads only the fields it needs.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Args {
    page: i32,
    query: String,
    manga: Option<Ref>,
    chapter: Option<Ref>,
}

#[derive(Deserialize)]
struct Ref {
    url: String,
}

#[derive(Deserialize)]
struct Envelope {
    #[serde(default)]
    args: Args,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HttpResponse {
    status: u16,
    body: String,
    #[serde(default)]
    body_encoding: Option<String>,
}

fn descriptor() -> Value {
    json!({ "id": ID, "name": NAME, "lang": LANG, "supportsLatest": true, "requestGapMs": REQUEST_GAP_MS })
}

/// The headers of a page image: the site as Referer.
fn image_headers() -> Value {
    json!([["Referer", site::BASE]])
}

/// Fetches a page of the site through the host, which refuses any host outside `hosts` in `manifest.toml`.
fn get(url: &str) -> Res<String> {
    let request = json!({ "method": "GET", "url": url, "headers": [["Referer", site::BASE]] });
    let response: HttpResponse = serde_json::from_str(&host::http(&request.to_string())?).map_err(|e| format!("invalid http reply: {e}"))?;
    if !(200..300).contains(&response.status) {
        return Err(format!("HTTP {} from {url}", response.status));
    }
    if response.body_encoding.is_some() {
        return Err(format!("{url} is not UTF-8 text"));
    }
    Ok(response.body)
}

fn to_value<T: serde::Serialize>(value: T) -> Res<Value> {
    serde_json::to_value(value).map_err(|e| e.to_string())
}

/// The search: a `POST` of the query with the token of the site's pages. The token and the session cookie come from the
/// first call, which the host keeps for the second.
fn search(query: &str) -> Res<site::MangasPage> {
    let token = site::csrf_token(&get(&site::top_url())?).ok_or("the page has no token")?;
    let body = url::form_urlencoded::Serializer::new(String::new()).append_pair("buscar", query.trim()).append_pair("_token", &token).finish();
    let request = json!({
        "method": "POST",
        "url": site::search_url(),
        "headers": [["Content-Type", "application/x-www-form-urlencoded"], ["X-Requested-With", "XMLHttpRequest"], ["Referer", site::top_url()]],
        "body": body,
    });
    let response: HttpResponse = serde_json::from_str(&host::http(&request.to_string())?).map_err(|e| format!("invalid http reply: {e}"))?;
    if !(200..300).contains(&response.status) {
        return Err(format!("HTTP {} from the MangaOni search", response.status));
    }
    site::parse_search(&response.body)
}

fn run(op: &str, args: Args) -> Res<Value> {
    match op {
        "popular" => to_value(site::parse_popular(&get(&site::top_url())?)),
        "latest" => to_value(site::parse_latest(&get(&site::recent_url())?)),
        // the site's search is a POST that needs a session cookie, which the plug-in does not hold
        "search" if args.query.trim().is_empty() => to_value(site::MangasPage { mangas: Vec::new(), has_next_page: false }),
        "search" => to_value(search(&args.query)?),
        "details" => {
            let manga = args.manga.ok_or("details needs args.manga")?;
            let mut found = site::parse_details(&get(&site::page(&manga.url)?)?)?;
            found.url = manga.url;
            to_value(found)
        }
        "chapters" => {
            let manga = args.manga.ok_or("chapters needs args.manga")?;
            to_value(site::parse_chapters(&get(&site::page(&manga.url)?)?))
        }
        "pages" => {
            let chapter = args.chapter.ok_or("pages needs args.chapter")?;
            let pages = site::parse_pages(&get(&site::page(&chapter.url)?)?)?;
            to_value(pages.into_iter().map(|image| json!({ "url": "", "imageUrl": image })).collect::<Vec<_>>())
        }
        _ => Err(format!("unknown op: {op}")),
    }
}

struct Plugin;

impl Guest for Plugin {
    fn invoke(op: String, input_json: String) -> Result<String, String> {
        let out = match op.as_str() {
            "info" => descriptor(),
            "create-sources" => Value::Array(vec![descriptor()]),
            // ABI 1.5: the page-image headers. They replace the app's default set for this source.
            "request-headers" => image_headers(),
            "popular" | "latest" | "search" | "details" | "chapters" | "pages" => {
                let envelope: Envelope = serde_json::from_str(&input_json).map_err(|e| format!("invalid input: {e}"))?;
                run(&op, envelope.args)?
            }
            _ => return Err(format!("unknown op: {op}")),
        };
        Ok(out.to_string())
    }
}

export!(Plugin);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_carries_the_id_and_the_spacing() {
        let info: Value = serde_json::from_str(&<Plugin as Guest>::invoke("info".into(), "{}".into()).unwrap()).unwrap();
        assert_eq!((info["id"].as_i64(), info["lang"].as_str(), info["requestGapMs"].as_u64()), (Some(ID), Some("es"), Some(1000)));
    }

    #[test]
    fn an_op_the_plug_in_does_not_answer_is_unknown() {
        assert_eq!(<Plugin as Guest>::invoke("hosters".into(), "{}".into()), Err("unknown op: hosters".to_owned()));
    }
}
