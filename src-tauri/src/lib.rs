//! Tauri shell: exposes the core library to the web front end.

use neojp_core::epwing::{self, EpwingStore};
use neojp_core::{
    DictionaryInfo, Entry, ExternalBodies, HistoryItem, Library, SearchHit, SearchQuery, yomitan,
};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager, State};

struct AppState {
    lib: Mutex<Library>,
    db_path: PathBuf,
    epwing: EpwingStore,
}

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EntryView {
    entry: Entry,
    prev: Option<SearchHit>,
    next: Option<SearchHit>,
    writing_mode: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportEvent {
    path: String,
    title: String,
    stage: String,
    done: usize,
    total: usize,
    error: Option<String>,
    finished: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct IndexEvent {
    dict_id: i64,
    title: String,
    done: usize,
    total: usize,
    finished: bool,
}

static INDEXING: AtomicBool = AtomicBool::new(false);
static INDEX_AGAIN: AtomicBool = AtomicBool::new(false);

/// Build the full-text index for every dictionary that lacks one, on a background thread.
fn spawn_indexer(app: AppHandle) {
    if INDEXING.swap(true, Ordering::SeqCst) {
        INDEX_AGAIN.store(true, Ordering::SeqCst);
        return;
    }
    std::thread::spawn(move || {
        let db_path = app.state::<AppState>().db_path.clone();
        loop {
            INDEX_AGAIN.store(false, Ordering::SeqCst);
            if let Ok(lib) = Library::open(&db_path) {
                let store = EpwingStore::default();
                for id in lib.fulltext_pending().unwrap_or_default() {
                    let Ok(d) = lib.dictionary(id) else { continue };
                    let mut last = std::time::Instant::now();
                    let _ = lib.build_fulltext(&d, Some(&store), |done, total| {
                        if last.elapsed().as_millis() > 300 {
                            last = std::time::Instant::now();
                            let _ = app.emit("index-progress", IndexEvent {
                                dict_id: d.id, title: d.title.clone(), done, total, finished: false,
                            });
                        }
                        true
                    });
                    let _ = app.emit("index-progress", IndexEvent {
                        dict_id: d.id, title: d.title.clone(), done: 0, total: 0, finished: true,
                    });
                }
            }
            if !INDEX_AGAIN.load(Ordering::SeqCst) {
                break;
            }
        }
        INDEXING.store(false, Ordering::SeqCst);
    });
}

#[tauri::command]
fn suggest(state: State<AppState>, text: String, dict_ids: Vec<i64>) -> CmdResult<Vec<String>> {
    state.lib.lock().unwrap().suggest(&text, &dict_ids, 10).map_err(err)
}

#[tauri::command]
fn fulltext_ready(state: State<AppState>) -> CmdResult<Vec<i64>> {
    state.lib.lock().unwrap().fulltext_ready().map_err(err)
}

#[tauri::command]
fn list_dictionaries(state: State<AppState>) -> CmdResult<Vec<DictionaryInfo>> {
    state.lib.lock().unwrap().dictionaries().map_err(err)
}

#[tauri::command]
fn search(state: State<AppState>, query: SearchQuery) -> CmdResult<Vec<SearchHit>> {
    state.lib.lock().unwrap().search_with(&query, Some(&state.epwing)).map_err(err)
}

#[tauri::command]
fn get_entry(state: State<AppState>, term_id: i64, record: bool) -> CmdResult<EntryView> {
    let lib = state.lib.lock().unwrap();
    let entry = lib.entry(term_id, Some(&state.epwing)).map_err(err)?;
    let (prev, next) = lib.neighbors(term_id).map_err(err)?;
    let writing_mode = lib.dictionary(entry.dict_id).map_err(err)?.writing_mode;
    if record {
        lib.add_history(term_id).map_err(err)?;
    }
    Ok(EntryView { entry, prev, next, writing_mode })
}

/// Follow a cross-reference inside an EPWING entry (`page:offset`).
#[tauri::command]
fn get_entry_at(state: State<AppState>, dict_id: i64, locator: String) -> CmdResult<EntryView> {
    let lib = state.lib.lock().unwrap();
    let entry = lib.entry_at(dict_id, &locator, Some(&state.epwing)).map_err(err)?;
    let (prev, next) = if entry.term_id > 0 { lib.neighbors(entry.term_id).map_err(err)? } else { (None, None) };
    if entry.term_id > 0 {
        lib.add_history(entry.term_id).map_err(err)?;
    }
    let writing_mode = lib.dictionary(dict_id).map_err(err)?.writing_mode;
    Ok(EntryView { entry, prev, next, writing_mode })
}

#[tauri::command]
fn browse(state: State<AppState>, dict_id: i64, after: Option<String>, offset: Option<usize>, limit: usize) -> CmdResult<Vec<SearchHit>> {
    state.lib.lock().unwrap().browse(dict_id, after.as_deref(), offset.unwrap_or(0), limit).map_err(err)
}

#[derive(Serialize)]
struct SpecialPage {
    kind: String,
    locator: String,
}

/// Menu, copyright and image-menu pages of an EPWING book (empty for other dictionaries).
#[tauri::command]
fn special_pages(state: State<AppState>, dict_id: i64) -> CmdResult<Vec<SpecialPage>> {
    let d = state.lib.lock().unwrap().dictionary(dict_id).map_err(err)?;
    if d.kind != "epwing" {
        return Ok(vec![]);
    }
    let pages = state.epwing.special_pages(&d).map_err(err)?;
    Ok(pages.into_iter().map(|(kind, locator)| SpecialPage { kind, locator }).collect())
}

#[tauri::command]
fn history(state: State<AppState>, limit: usize) -> CmdResult<Vec<HistoryItem>> {
    state.lib.lock().unwrap().history(limit).map_err(err)
}

#[tauri::command]
fn clear_history(state: State<AppState>) -> CmdResult<()> {
    state.lib.lock().unwrap().clear_history().map_err(err)
}

#[tauri::command]
fn bookmarks(state: State<AppState>) -> CmdResult<Vec<HistoryItem>> {
    state.lib.lock().unwrap().bookmarks().map_err(err)
}

#[tauri::command]
fn markers(state: State<AppState>, term_id: i64) -> CmdResult<Vec<neojp_core::library::Marker>> {
    state.lib.lock().unwrap().markers(term_id).map_err(err)
}

#[tauri::command]
fn all_markers(state: State<AppState>) -> CmdResult<Vec<neojp_core::library::MarkerRow>> {
    state.lib.lock().unwrap().all_markers().map_err(err)
}

#[tauri::command]
fn add_marker(state: State<AppState>, term_id: i64, text: String, color: String) -> CmdResult<i64> {
    state.lib.lock().unwrap().add_marker(term_id, &text, &color).map_err(err)
}

#[tauri::command]
fn remove_marker(state: State<AppState>, id: i64) -> CmdResult<()> {
    state.lib.lock().unwrap().remove_marker(id).map_err(err)
}

#[tauri::command]
fn set_bookmark(state: State<AppState>, term_id: i64, on: bool) -> CmdResult<()> {
    state.lib.lock().unwrap().set_bookmark(term_id, on).map_err(err)
}

#[tauri::command]
fn update_bookmark(state: State<AppState>, term_id: i64, folder: String, note: String) -> CmdResult<()> {
    state.lib.lock().unwrap().update_bookmark(term_id, &folder, &note).map_err(err)
}

#[tauri::command]
fn export_bookmarks(state: State<AppState>, path: String) -> CmdResult<()> {
    let csv = state.lib.lock().unwrap().bookmarks_csv().map_err(err)?;
    std::fs::write(path, csv).map_err(err)
}

#[tauri::command]
fn backup_userdata(state: State<AppState>, path: String) -> CmdResult<()> {
    let json = state.lib.lock().unwrap().backup_userdata().map_err(err)?;
    std::fs::write(path, json).map_err(err)
}

#[tauri::command]
fn restore_userdata(state: State<AppState>, path: String) -> CmdResult<neojp_core::userdata::RestoreSummary> {
    let json = std::fs::read_to_string(path).map_err(err)?;
    state.lib.lock().unwrap().restore_userdata(&json).map_err(err)
}

#[tauri::command]
fn delete_dictionary(state: State<AppState>, id: i64) -> CmdResult<()> {
    state.lib.lock().unwrap().delete_dictionary(id).map_err(err)
}

#[tauri::command]
fn update_dictionary(
    state: State<AppState>,
    id: i64,
    enabled: bool,
    group_name: String,
    writing_mode: String,
) -> CmdResult<()> {
    state
        .lib
        .lock()
        .unwrap()
        .update_dictionary_settings(id, enabled, &group_name, &writing_mode)
        .map_err(err)
}

#[tauri::command]
fn update_dictionary_display(state: State<AppState>, id: i64, alias: String, font: String) -> CmdResult<()> {
    state.lib.lock().unwrap().update_dictionary_display(id, &alias, &font).map_err(err)
}

#[tauri::command]
fn reorder_dictionaries(state: State<AppState>, ids: Vec<i64>) -> CmdResult<()> {
    state.lib.lock().unwrap().reorder_dictionaries(&ids).map_err(err)
}

/// Import dictionaries on a background thread. Progress arrives as `import-progress` events.
#[tauri::command]
fn import_dictionaries(app: AppHandle, state: State<AppState>, paths: Vec<String>) -> CmdResult<()> {
    let db_path = state.db_path.clone();
    std::thread::spawn(move || {
        // A second connection lets searches keep running while the import writes (WAL mode).
        let lib = match Library::open(&db_path) {
            Ok(l) => l,
            Err(e) => {
                let _ = app.emit("import-progress", ImportEvent {
                    path: String::new(),
                    title: String::new(),
                    stage: String::new(),
                    done: 0,
                    total: 0,
                    error: Some(e.to_string()),
                    finished: true,
                });
                return;
            }
        };
        for path in paths {
            let result = import_one(&app, &lib, &path);
            let _ = app.emit("import-progress", ImportEvent {
                path: path.clone(),
                title: String::new(),
                stage: "done".into(),
                done: 0,
                total: 0,
                error: result.err(),
                finished: true,
            });
        }
        spawn_indexer(app.clone());
    });
    Ok(())
}

fn import_one(app: &AppHandle, lib: &Library, path: &str) -> Result<(), String> {
    let p = Path::new(path);
    let report = |prog: &yomitan::ImportProgress| {
        let _ = app.emit("import-progress", ImportEvent {
            path: path.to_string(),
            title: prog.title.clone(),
            stage: prog.stage.clone(),
            done: prog.done,
            total: prog.total,
            error: None,
            finished: false,
        });
        true
    };
    let is_zip = p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("zip"));
    if is_zip {
        return yomitan::import(lib, p, report).map(|_| ()).map_err(err);
    }
    // A folder (or a CATALOGS file): import every EPWING book found in it.
    let books = epwing::find_books(p);
    if books.is_empty() {
        return Err("Not a dictionary: choose a Yomitan .zip file or a folder with an EPWING CATALOGS file.".into());
    }
    let mut errors = vec![];
    for book in books {
        if let Err(e) = epwing::import(lib, &book, report) {
            errors.push(format!("{}: {e}", book.file_name().unwrap_or_default().to_string_lossy()));
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors.join("\n")) }
}

fn media_response(app: &AppHandle, path: &str) -> tauri::http::Response<Vec<u8>> {
    let not_found = || {
        tauri::http::Response::builder()
            .status(404)
            .body(Vec::new())
            .unwrap()
    };
    // `/<dict id>/<path inside the dictionary>`
    let path = path.trim_start_matches('/');
    let Some((id, rest)) = path.split_once('/') else { return not_found() };
    let Ok(dict_id) = id.parse::<i64>() else { return not_found() };
    let rest = percent_decode(rest);
    let state = app.state::<AppState>();
    let data = {
        let lib = state.lib.lock().unwrap();
        match lib.media(dict_id, &rest).ok().flatten() {
            Some(d) => Some(d),
            None => lib
                .dictionary(dict_id)
                .ok()
                .filter(|d| d.kind == "epwing")
                .and_then(|d| state.epwing.media(&d, &rest).ok().flatten()),
        }
    };
    match data {
        Some(bytes) => {
            let mime = match rest.rsplit('.').next().map(|e| e.to_ascii_lowercase()).as_deref() {
                _ if data_is_jpeg(&bytes) => "image/jpeg",
                _ if bytes.starts_with(b"BM") => "image/bmp",
                Some("png") => "image/png",
                Some("jpg" | "jpeg") => "image/jpeg",
                Some("gif") => "image/gif",
                Some("svg") => "image/svg+xml",
                Some("webp") => "image/webp",
                Some("avif") => "image/avif",
                Some("bmp") => "image/bmp",
                Some("mp3") => "audio/mpeg",
                Some("ogg" | "opus") => "audio/ogg",
                Some("wav") => "audio/wav",
                _ => "application/octet-stream",
            };
            tauri::http::Response::builder()
                .header("Content-Type", mime)
                .header("Cache-Control", "max-age=31536000, immutable")
                // The page is on another origin, and CSS masks (monochrome images) load with CORS.
                .header("Access-Control-Allow-Origin", "*")
                .body(bytes)
                .unwrap()
        }
        None => not_found(),
    }
}

fn data_is_jpeg(b: &[u8]) -> bool {
    b.starts_with(&[0xff, 0xd8])
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Bring the main window to the front (restoring it if minimised).
fn bring_forward(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Set the system-wide shortcut that brings NeoJPDict forward and looks up the clipboard.
/// An empty string turns it off.
#[tauri::command]
fn set_global_shortcut(app: AppHandle, accelerator: String) -> CmdResult<()> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
    let gs = app.global_shortcut();
    gs.unregister_all().map_err(err)?;
    let accelerator = accelerator.trim();
    if accelerator.is_empty() {
        return Ok(());
    }
    let shortcut: Shortcut = accelerator.parse().map_err(|e| format!("{e}"))?;
    gs.register(shortcut).map_err(err)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tauri_plugin_global_shortcut::ShortcutState;
    tauri::Builder::default()
        // A second launch focuses the running window instead of opening another.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| bring_forward(app)))
        // neojpdict://search/<word> links (the installer registers the scheme).
        .plugin(tauri_plugin_deep_link::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        bring_forward(app);
                        let _ = app.emit("global-lookup", ());
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .register_uri_scheme_protocol("njmedia", |ctx, request| {
            media_response(ctx.app_handle(), request.uri().path())
        })
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db_path = dir.join("library.sqlite3");
            let lib = Library::open(&db_path).map_err(|e| e.to_string())?;
            app.manage(AppState { lib: Mutex::new(lib), db_path, epwing: EpwingStore::default() });
            spawn_indexer(app.handle().clone());
            // Also register the link scheme at run time, so a copied or portable app works.
            #[cfg(desktop)]
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                let _ = app.deep_link().register_all();
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_dictionaries,
            fulltext_ready,
            suggest,
            search,
            get_entry,
            get_entry_at,
            browse,
            special_pages,
            history,
            clear_history,
            bookmarks,
            set_bookmark,
            markers,
            add_marker,
            all_markers,
            remove_marker,
            update_bookmark,
            export_bookmarks,
            backup_userdata,
            restore_userdata,
            delete_dictionary,
            update_dictionary,
            reorder_dictionaries,
            update_dictionary_display,
            set_global_shortcut,
            import_dictionaries,
        ])
        .run(tauri::generate_context!())
        .expect("error while running NeoJPDict");
}
