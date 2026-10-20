# LeerCapitulo

LeerCapitulo (<https://www.leercapitulo.co>) as a Spanish manga source plug-in for Kurobayomi (ABI 1.5,
`wit/kuro-source.wit`, manga ops).

## Origin and licence

Written for this repository from the site's own pages. It is not a port of an existing extension. Licensed under the
Apache License 2.0, like the other sources. The source id is `MD5("leercapitulo/es/1")` taken as its first 8 bytes,
big-endian, with the top bit cleared: `5621901596522308538`.

## Ops

| Op | Request |
|---|---|
| `info`, `create-sources` | the descriptor; Latest; 1 s between requests |
| `request-headers` | the page-image headers: the site as Referer |
| `popular` | `GET /manga/?page=N`: the directory, A to Z (the site has no popularity list) |
| `latest` | `GET /`: the latest updates in the home slider, one manga per slide (one page) |
| `search` | `GET /manga/?name=...&page=N`; there is a next page when a link names page N+1 |
| `details` | `GET /manga/{id}/{slug}/`: title, summary (`#sinopsis`), genres, status, cover |
| `chapters` | the same page: `#chapterList` rows, newest first, with the number and the date (`YYYY-MM-DD`) |
| `pages` | `GET /leer/{id}/{slug}/{chapter}/`: the `#lcPages` images in order (`data-src`) |

## Not ported

- The directory filters (genre, status, type, demographic, sort).
- The author and artist: the details do not carry them.

## Test

```bash
cd sources/leercapitulo && cargo test
```

The tests parse the fixtures in `fixtures/`. They are hand-written pages with invented titles, not copies of the site,
so they show the structure the parsers rely on and nothing more.

## Build

From the repository root, see `tools/build.sh` (it builds every plug-in) or build this one by itself with
`CARGO_TARGET_DIR=target CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER=wasm-component-ld cargo build --release --target wasm32-unknown-unknown`
from this folder.
