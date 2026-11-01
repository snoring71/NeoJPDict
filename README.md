# NeoJPDict

A Windows 11 dictionary app for Japanese that reads **Yomitan/Yomichan** and **EPWING** dictionaries and shows entries in **vertical text (縦書き)**. The layout takes its cues from 物書堂's 辞書 app: search and per-dictionary results on the left, a book-like reader on the right.

> Status: early development, but feature-complete for everyday use.

## Features

**Dictionaries**
- Imports Yomitan/Yomichan `.zip` dictionaries (formats 1–3). That covers terms, kanji, tags, frequency and pitch-accent data, images and dictionary styles.
- Reads EPWING books in place: gaiji, colour and monochrome graphics, sound, cross-references, and the book's menu and copyright pages.
- Groups (国語, 和英 …), reordering, renaming, a font per dictionary and a text direction per dictionary.

**Search**
- One search covers every dictionary, with results grouped by dictionary.
- 前方 / 完全 / 後方 / 部分 matching, `*` / `?` wildcards and regular expressions.
- Kana and width folding (ベンリ = べんり = ﾍﾞﾝﾘ), romaji input (`kakeru`) and deinflection (食べなかった → 食べる).
- 慣用句 (idioms), 用例 (examples) and 全文 (full-text) search, with the matching text shown under each hit.
- "Did you mean" suggestions, and finding kanji by their parts (部品).

**Reader**
- Vertical text (縦書き) that scrolls right to left. It falls back to horizontal for English-heavy entries and tables, and you can switch either way.
- Marks such as brackets and accent drops are turned for vertical text.
- Select-to-look-up, highlighter markers, find in entry, back / forward, and copy and print.
- Pitch accent and frequency are shown in the entry header.

**Everyday use**
- History by day with lookup counts. Bookmarks with folders and notes, exported as CSV.
- Backup and restore of bookmarks, markers and history.
- A system-wide lookup shortcut (Ctrl+Alt+J) and clipboard lookup.
- `neojpdict://search/言葉` links, keep-on-top and a compact window.
- Japanese and English UI, light and dark themes, paper colours, and the Mica backdrop on Windows 11.

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
