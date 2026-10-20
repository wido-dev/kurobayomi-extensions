//! JKAnime (<https://jkanime.net>), a Spanish anime source plug-in for Kurobayomi, ABI 1.5 (`wit/kuro-source.wit`). This file
//! is the ABI glue: the ops and the one network path, the `http` import. The URLs and parsers are in `site.rs`.

mod site;

use serde::Deserialize;
use serde_json::{Value, json};

wit_bindgen::generate!({
    path: "wit",
    world: "source",
});

use kuro::source::host;
use site::{Anime, Episode, Hoster, Res, Video};

/// The source id the app derives for it: `generate_id("JKAnime", "es", 1)`, the MD5 of `jkanime/es/1`. Keep it: backups and
/// the library name it.
const ID: i64 = 6007374093317538838;
const NAME: &str = "JKAnime";
const LANG: &str = "es";
/// One request a second keeps well under any limit the site may have.
const REQUEST_GAP_MS: u64 = 1000;
/// When a title page gives no episode count, the episode search is asked for `1`, `2`, ... until it has no episode. This
/// caps the number of requests, so one title cannot take the app's whole time.
const MAX_PROBED_EPISODES: u32 = 300;

/// The `args` of an op. Each op reads only the fields it needs.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Args {
    query: String,
    anime: Option<Anime>,
    episode: Option<Episode>,
    hoster: Option<Hoster>,
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
    json!({ "id": ID, "name": NAME, "lang": LANG, "supportsLatest": false, "requestGapMs": REQUEST_GAP_MS })
}

/// One GET of a site page through the host, which refuses any host outside `hosts` in `manifest.toml`.
fn get(url: &str) -> Res<String> {
    let request = json!({ "method": "GET", "url": url, "headers": [["Referer", site::absolute("/")]] });
    let response: HttpResponse = serde_json::from_str(&host::http(&request.to_string())?).map_err(|e| format!("invalid http reply: {e}"))?;
    if response.body_encoding.is_some() {
        return Err(format!("{url} sent a binary body"));
    }
    match response.status {
        200..=299 => Ok(response.body),
        429 => Err("JKAnime is limiting requests; wait a few seconds and try again".into()),
        status => Err(format!("HTTP {status} from JKAnime")),
    }
}

fn page(path: &str) -> Res<String> {
    get(&site::page(path)?)
}

fn to_value<T: serde::Serialize>(value: T) -> Res<Value> {
    serde_json::to_value(value).map_err(|e| e.to_string())
}

/// One POST of a form to the site, with the token and the session the host keeps from the title page.
fn post(url: &str, form: &str, referer: &str, token: &str) -> Res<String> {
    let request = json!({
        "method": "POST",
        "url": url,
        "headers": [
            ["Content-Type", "application/x-www-form-urlencoded"],
            ["X-Requested-With", "XMLHttpRequest"],
            ["X-CSRF-TOKEN", token],
            ["Referer", referer],
        ],
        "body": form,
    });
    let response: HttpResponse = serde_json::from_str(&host::http(&request.to_string())?).map_err(|e| format!("invalid http reply: {e}"))?;
    match response.status {
        200..=299 => Ok(response.body),
        419 => Err("JKAnime's session expired; try again".into()),
        status => Err(format!("HTTP {status} from JKAnime")),
    }
}

/// The episodes of a title. The count on its page gives them directly. When the page gives no count, the site's episode
/// search is asked for `1`, `2`, ... until it answers with no episode. A failed request is an error, not the end of the list.
fn episodes_of(anime_url: &str) -> Res<Vec<Episode>> {
    let html = page(anime_url)?;
    let count = site::episode_count(&html);
    if count > 0 {
        return Ok(site::episodes(anime_url, count));
    }
    let Some(id) = site::anime_id(&html) else {
        return Ok(Vec::new());
    };
    let token = site::csrf_token(&html).ok_or("the title page has no token")?;
    let referer = site::absolute(anime_url);
    let search = format!("{}/ajax/search_episode/{id}", site::BASE);
    let mut found = 0;
    while found < MAX_PROBED_EPISODES {
        let form = url::form_urlencoded::Serializer::new(String::new()).append_pair("_token", &token).finish();
        let reply = post(&format!("{search}/{}", found + 1), &form, &referer, &token)?;
        if !site::episode_found(&reply)? {
            break;
        }
        found += 1;
    }
    Ok(site::episodes(anime_url, found))
}

/// The playable address of one player, read from its page. The page's own video address is short-lived, so it is read
/// each time the app plays the episode.
fn video_of(hoster: &Hoster) -> Res<Video> {
    let html = get(&hoster.hoster_url)?;
    let video_url = site::parse_playlist(&html).ok_or_else(|| format!("{} has no playlist", hoster.hoster_name))?;
    Ok(Video {
        video_url,
        video_title: hoster.hoster_name.clone(),
        resolution: None,
        headers: site::video_headers(),
        preferred: true,
    })
}

fn run(op: &str, args: Args) -> Res<Value> {
    match op {
        "popular" => to_value(site::parse_popular(&get(&site::home_url())?)),
        // the empty search lists the featured titles, as the other sources do
        "search" if args.query.trim().is_empty() => to_value(site::parse_popular(&get(&site::home_url())?)),
        "search" => to_value(site::parse_search(&get(&site::search_url(&args.query))?)),
        "details" => {
            let anime = args.anime.ok_or("details needs args.anime")?;
            to_value(site::parse_details(&page(&anime.url)?, &anime.url)?)
        }
        "episodes" => to_value(episodes_of(&args.anime.ok_or("episodes needs args.anime")?.url)?),
        "hosters" => {
            let episode = args.episode.ok_or("hosters needs args.episode")?;
            to_value(site::parse_hosters(&page(&episode.url)?))
        }
        "videos" => match (args.hoster, args.episode) {
            (Some(hoster), _) => to_value(vec![video_of(&hoster)?]),
            (None, Some(episode)) => {
                let hosters = site::parse_hosters(&page(&episode.url)?);
                let videos: Vec<Video> = hosters
                    .iter()
                    .filter_map(|hoster| match video_of(hoster) {
                        Ok(video) => Some(video),
                        Err(error) => {
                            host::log(2, &format!("jkanime: {} failed: {error}", hoster.hoster_name));
                            None
                        }
                    })
                    .collect();
                to_value(videos)
            }
            (None, None) => Err("videos needs a hoster or an episode".into()),
        },
        _ => Err(format!("unknown op: {op}")),
    }
}

struct Plugin;

impl Guest for Plugin {
    fn invoke(op: String, input_json: String) -> Result<String, String> {
        let out = match op.as_str() {
            "info" => descriptor(),
            "create-sources" => Value::Array(vec![descriptor()]),
            // ABI 1.5: the video host needs the site as referer
            "request-headers" => to_value(site::video_headers())?,
            "popular" | "search" | "details" | "episodes" | "hosters" | "videos" => {
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
    fn info_carries_the_id_and_no_latest_list() {
        let info: Value = serde_json::from_str(&<Plugin as Guest>::invoke("info".into(), "{}".into()).unwrap()).unwrap();
        assert_eq!((info["id"].as_i64(), info["lang"].as_str(), info["supportsLatest"].as_bool()), (Some(ID), Some("es"), Some(false)));
    }

    #[test]
    fn an_op_the_plug_in_does_not_answer_is_unknown() {
        assert_eq!(<Plugin as Guest>::invoke("latest".into(), "{}".into()), Err("unknown op: latest".to_owned()));
    }
}
