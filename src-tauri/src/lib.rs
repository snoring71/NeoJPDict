//! Tauri shell: exposes the core library to the web front end.

use neojp_core::{
    DictionaryInfo, Entry, HistoryItem, Library, SearchHit, SearchQuery, yomitan,
};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

struct AppState {
    lib: Mutex<Library>,
    db_path: PathBuf,
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

#[tauri::command]
fn list_dictionaries(state: State<AppState>) -> CmdResult<Vec<DictionaryInfo>> {
    state.lib.lock().unwrap().dictionaries().map_err(err)
}

#[tauri::command]
fn search(state: State<AppState>, query: SearchQuery) -> CmdResult<Vec<SearchHit>> {
    state.lib.lock().unwrap().search(&query).map_err(err)
}

#[tauri::command]
fn get_entry(state: State<AppState>, term_id: i64, record: bool) -> CmdResult<EntryView> {
    let lib = state.lib.lock().unwrap();
    let entry = lib.entry(term_id, None).map_err(err)?;
    let (prev, next) = lib.neighbors(term_id).map_err(err)?;
    let writing_mode = lib.dictionary(entry.dict_id).map_err(err)?.writing_mode;
    if record {
        lib.add_history(term_id).map_err(err)?;
    }
    Ok(EntryView { entry, prev, next, writing_mode })
}

#[tauri::command]
fn browse(state: State<AppState>, dict_id: i64, after: Option<String>, limit: usize) -> CmdResult<Vec<SearchHit>> {
    state.lib.lock().unwrap().browse(dict_id, after.as_deref(), limit).map_err(err)
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
fn set_bookmark(state: State<AppState>, term_id: i64, on: bool) -> CmdResult<()> {
    state.lib.lock().unwrap().set_bookmark(term_id, on).map_err(err)
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
    });
    Ok(())
}

fn import_one(app: &AppHandle, lib: &Library, path: &str) -> Result<(), String> {
    let p = Path::new(path);
    let is_zip = p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("zip"));
    let is_epwing = p.is_dir() && (p.join("CATALOGS").exists() || p.join("catalogs").exists());
    if is_epwing || p.file_name().is_some_and(|n| n.eq_ignore_ascii_case("CATALOGS")) {
        return Err("EPWING import is coming in the next update.".into());
    }
    if !is_zip {
        return Err("Not a dictionary: choose a Yomitan .zip file or an EPWING folder.".into());
    }
    yomitan::import(lib, p, |prog| {
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
    })
    .map(|_| ())
    .map_err(err)
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
    let data = state.lib.lock().unwrap().media(dict_id, &rest).ok().flatten();
    match data {
        Some(bytes) => {
            let mime = match rest.rsplit('.').next().map(|e| e.to_ascii_lowercase()).as_deref() {
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
                .body(bytes)
                .unwrap()
        }
        None => not_found(),
    }
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .register_uri_scheme_protocol("njmedia", |ctx, request| {
            media_response(ctx.app_handle(), request.uri().path())
        })
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db_path = dir.join("library.sqlite3");
            let lib = Library::open(&db_path).map_err(|e| e.to_string())?;
            app.manage(AppState { lib: Mutex::new(lib), db_path });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_dictionaries,
            search,
            get_entry,
            browse,
            history,
            clear_history,
            bookmarks,
            set_bookmark,
            delete_dictionary,
            update_dictionary,
            reorder_dictionaries,
            import_dictionaries,
        ])
        .run(tauri::generate_context!())
        .expect("error while running NeoJPDict");
}
