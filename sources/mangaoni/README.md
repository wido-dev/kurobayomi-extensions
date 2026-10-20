# MangaOni

MangaOni (<https://manga-oni.com>) as a Spanish manga source plug-in for Kurobayomi (ABI 1.5, `wit/kuro-source.wit`, manga ops).

## Origin and licence

Written for this repository from the site's own pages. It is not a port of an existing extension. Licensed under the Apache
License 2.0, like the other sources. The source id is `MD5("mangaoni/es/1")` taken as its first 8 bytes, big-endian, with the
top bit cleared: `2342274476374431513`.

## Ops

| Op | Request |
|---|---|
| `info`, `create-sources` | the descriptor; Latest; 1 s between requests |
| `request-headers` | the page-image headers: the site as Referer |
| `popular` | `GET /top`: the most visited titles (one page, no paging) |
| `latest` | `GET /recientes`: the recently updated titles (one page, no paging) |
| `search` | `GET /` for the page token, then `POST /buscar` with `buscar` and `_token`. The reply is JSON; hidden titles are left out. An empty query gives no titles |
| `details` | `GET /manga/<slug>/`: title, summary (`#sinopsis`), genres, status and cover |
| `chapters` | the same page: `#c_list`, newest first, with the number and the date from the `timeago` element |
| `pages` | `GET /lector/<slug>/<id>/`: the page list, which the reader script holds base64-encoded in `unicap` |

## Caveats

- The search depends on the session the host keeps between the home page and the POST, through the shared cookie jar of the
  app. It is not tested in the app yet.
- The page list is read from the chapter page itself, so no second request is needed for the images.

## Test

```bash
cd sources/mangaoni && cargo test
```

The tests parse the fixtures in `fixtures/`. They are hand-written pages and replies with invented titles, not copies of the
site.
