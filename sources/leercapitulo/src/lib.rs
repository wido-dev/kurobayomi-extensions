//! LeerCapitulo (<https://www.leercapitulo.co>), a Spanish manga source plug-in for Kurobayomi, ABI 1.5
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

/// The source id the app derives for it: `generate_id("LeerCapitulo", "es", 1)`, the MD5 of `leercapitulo/es/1`. Keep it:
/// backups and the library name it.
const ID: i64 = 5621901596522308538;
const NAME: &str = "LeerCapitulo";
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

fn run(op: &str, args: Args) -> Res<Value> {
    let page = args.page.max(1);
    match op {
        "popular" => to_value(site::parse_popular(&get(&site::directory_url(page))?, page)),
        "latest" => to_value(site::parse_latest(&get(&site::home_url())?)),
        "search" => to_value(site::parse_search(&get(&site::search_url(page, &args.query))?, page)),
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
            let pages = site::parse_pages(&get(&site::page(&chapter.url)?)?);
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
