# NeoJPDict

A Windows 11 dictionary app for Japanese that reads **Yomitan/Yomichan** and **EPWING** dictionaries and shows entries in **vertical text (縦書き)**. The layout takes its cues from 物書堂's 辞書 app: search and per-dictionary results on the left, a book-like reader on the right.

> Status: early development. Yomitan import, search and the vertical reader work. EPWING support is the next milestone.

## Features

- Import Yomitan/Yomichan `.zip` dictionaries (formats 1–3): terms, kanji, tags, frequency and pitch-accent data, images and dictionary styles.
- One search across every dictionary, grouped by dictionary, with 前方/完全/後方/部分 matching, kana and width folding (ベンリ = べんり = ﾍﾞﾝﾘ) and deinflection (食べなかった → 食べる).
- Vertical reader with right-to-left scrolling (mouse wheel moves through the columns), automatic fallback to horizontal for English-heavy entries or tables, and a per-entry and per-dictionary override.
- Pitch accent and frequency shown in the entry header; links inside entries jump to that word.
- History, bookmarks, browsing a dictionary in reading order, previous/next headword.
- Dictionary groups (国語, 和英 …) as tabs, reordering, enable/disable.
- Japanese and English UI, light and dark themes, Mica backdrop on Windows 11.

## Download

Every push builds a Windows installer in GitHub Actions: open the latest **CI** run and download the `NeoJPDict-windows-installer` artifact.

## Building from source

Requirements: Rust (stable), Node.js 22, and on Windows the WebView2 runtime (included in Windows 11).

```sh
npm install
npm run tauri dev      # run the app with hot reload
npm run tauri build    # build an installer under target/release/bundle
cargo test -p neojp-core
```

`npm run dev` alone opens the UI in a normal browser with a small built-in sample dictionary, which is handy for UI work.

## Layout

| Path | What it is |
| --- | --- |
| `crates/neojp-core` | Format readers, SQLite library, search and deinflection. No UI code. |
| `src-tauri` | The Tauri shell: commands, image protocol, window setup. |
| `src` | Svelte front end. |

The library lives in `%APPDATA%\io.github.snoring71.neojpdict\library.sqlite3`.

## Licence

GPL-3.0-or-later. Dictionaries are not included; the app only reads dictionaries you already have.

`src/assets/kanji-parts.json` (the data behind “Find kanji by parts”) is derived from
[KanjiVG](https://kanjivg.tagaini.net/) © Ulrich Apel (CC BY-SA 3.0) and
[KANJIDIC2](https://www.edrdg.org/wiki/index.php/KANJIDIC_Project) © the Electronic
Dictionary Research and Development Group (CC BY-SA 4.0), by way of the `kanji` npm
package. That file is distributed under CC BY-SA 4.0.
