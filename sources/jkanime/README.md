# JKAnime

JKAnime (<https://jkanime.net>) as a Spanish anime source plug-in for Kurobayomi (ABI 1.5, `wit/kuro-source.wit`, anime ops).

## Origin and licence

Written for this repository from the site's own pages. It is not a port of an existing extension. Licensed under the Apache
License 2.0, like the other sources. The source id is `MD5("jkanime/es/1")` taken as its first 8 bytes, big-endian, with the
top bit cleared: `6007374093317538838`.

## Ops

| Op | Request |
|---|---|
| `info`, `create-sources` | the descriptor; no latest list; 1 s between requests |
| `request-headers` | the site as Referer for the video requests |
| `popular` | `GET /`: the featured titles of the home hero (one page, no paging) |
| `search` | `GET /buscar/<text>/`: the title cards (no next page). An empty query shows the featured titles |
| `details` | `GET /<slug>/`: title, summary (meta description), genres, status and cover |
| `episodes` | the same page: the count from "Episodios:" gives `/<slug>/1` to `/<slug>/N`. When the count is 0, the plug-in asks the site's episode search (`POST /ajax/search_episode/<id>/<n>`, with the page's token and session) for `1`, `2`, ... and stops at the first empty answer, at most 300 episodes |
| `hosters` | `GET /<slug>/<n>`: the site's own players only (`jkplayer/um` and `jkplayer/umv`) |
| `videos` | `GET` the player: the first HLS playlist address (`.m3u8`) it holds |

## Not ported

- The "latest" list: the site has no list the plug-in can read. The app shows no latest tab for this source.
- The other players of the site (`jkplayer/c1`) and the external servers (Mediafire, Mega, Streamwish, Vidhide, VOE,
  Filemoon, Mixdrop, Streamtape). Their pages need per-host extractors.
- Pages of search results and the directory filters.

## Caveats

- The playlist address of a player is short-lived. It is read again each time an episode is played.
- The episode count comes from the title page. A title that has not published its count gets one POST per episode, which
  takes one second per request.
- The episode search depends on the session the host keeps between the title page and the POST. If the site answers 419,
  the op fails with a message to try again.

## Test

```bash
cd sources/jkanime && cargo test
```

The tests parse the fixtures in `fixtures/`. They are hand-written pages with invented titles, not copies of the site.
