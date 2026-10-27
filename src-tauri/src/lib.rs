//! Zarządca — aplikacja na komputer.
//! Interfejs to ten sam plik HTML co wersja w przeglądarce (dist/index.html).
//! Tu jest tylko to, czego przeglądarka nie potrafi: stały folder danych (np. w OneDrive),
//! bezpieczny zapis plików, wybór folderu, „Pokaż w Eksploratorze” i aktualizacje.

use std::{
    fs,
    io::ErrorKind,
    path::{Component, Path, PathBuf},
    sync::Mutex,
    time::UNIX_EPOCH,
};

use serde::{Deserialize, Serialize};
use tauri::{
    ipc::{InvokeBody, Request, Response},
    AppHandle, Emitter, Manager, State,
};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_updater::UpdaterExt;

#[derive(Default, Serialize, Deserialize, Clone)]
struct Cfg {
    root: Option<String>,
    user: Option<String>,
    /// prywatny folder właściciela na wyniki finansowe (P&L) — poza wspólnym folderem danych
    #[serde(default)]
    pnl: Option<String>,
}

struct AppState {
    cfg: Mutex<Cfg>,
    pending: Mutex<Option<tauri_plugin_updater::Update>>,
}

type Res<T> = Result<T, String>;
fn es<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/* ---------- ustawienia (folder danych, imię) ---------- */

fn cfg_file(app: &AppHandle) -> Res<PathBuf> {
    let dir = app.path().app_config_dir().map_err(es)?;
    fs::create_dir_all(&dir).map_err(es)?;
    Ok(dir.join("ustawienia.json"))
}

fn load_cfg(app: &AppHandle) -> Cfg {
    cfg_file(app)
        .ok()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save_cfg(app: &AppHandle, cfg: &Cfg) -> Res<()> {
    let txt = serde_json::to_string_pretty(cfg).map_err(es)?;
    write_atomic(&cfg_file(app)?, txt.as_bytes())
}

/// Nazwa komputera na Macu (np. „MacBook Anny”) — aplikacje z Docka nie dostają zmiennej HOSTNAME.
#[cfg(target_os = "macos")]
fn mac_name() -> Option<String> {
    let o = std::process::Command::new("scutil").args(["--get", "ComputerName"]).output().ok()?;
    let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
    if o.status.success() && !s.is_empty() { Some(s) } else { None }
}
#[cfg(not(target_os = "macos"))]
fn mac_name() -> Option<String> {
    None
}

/// OneDrive na Macu: ~/Library/CloudStorage/OneDrive-Firma lub OneDrive-Personal (starsze wersje: ~/OneDrive).
#[cfg(target_os = "macos")]
fn mac_onedrive() -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var("HOME").ok()?);
    let mut found: Vec<PathBuf> = fs::read_dir(home.join("Library/CloudStorage"))
        .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.is_dir() && p.file_name().map(|n| n.to_string_lossy().starts_with("OneDrive")).unwrap_or(false)).collect())
        .unwrap_or_default();
    found.sort();
    let business = found.iter().find(|p| !p.to_string_lossy().ends_with("OneDrive-Personal")).cloned();
    business.or_else(|| found.first().cloned()).or_else(|| Some(home.join("OneDrive")).filter(|p| p.is_dir()))
}
#[cfg(not(target_os = "macos"))]
fn mac_onedrive() -> Option<PathBuf> {
    None
}

fn machine_name() -> String {
    if let Some(n) = mac_name() {
        return n;
    }
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| fs::read_to_string("/etc/hostname").ok().map(|s| s.trim().to_string()))
        .unwrap_or_else(|| "komputer".into())
}

/// Proponowany folder: OneDrive (firmowy, potem prywatny), a gdy go nie ma — Dokumenty.
fn suggest_root(app: &AppHandle) -> String {
    for key in ["OneDriveCommercial", "OneDriveConsumer", "OneDrive"] {
        if let Ok(v) = std::env::var(key) {
            if !v.is_empty() && Path::new(&v).is_dir() {
                return Path::new(&v).join("Zarządca").to_string_lossy().into_owned();
            }
        }
    }
    if let Some(p) = mac_onedrive() {
        return p.join("Zarządca").to_string_lossy().into_owned();
    }
    app.path()
        .document_dir()
        .map(|d| d.join("Zarządca").to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[derive(Serialize)]
struct CfgOut {
    root: Option<String>,
    user: Option<String>,
    suggest: String,
    machine: String,
    version: String,
    pnl: Option<String>,
    pnl_suggest: String,
}

#[tauri::command]
fn cfg_get(app: AppHandle, st: State<'_, AppState>) -> CfgOut {
    let c = st.cfg.lock().unwrap().clone();
    CfgOut {
        root: c.root.clone(),
        user: c.user,
        suggest: suggest_root(&app),
        machine: machine_name(),
        version: app.package_info().version.to_string(),
        pnl_suggest: c
            .root
            .as_ref()
            .and_then(|r| Path::new(r).parent().map(|p| p.join("Zarządca-finanse").to_string_lossy().into_owned()))
            .or_else(|| app.path().document_dir().ok().map(|d| d.join("Zarządca-finanse").to_string_lossy().into_owned()))
            .unwrap_or_default(),
        pnl: c.pnl,
    }
}

/* ---------- prywatne dane finansowe właściciela (P&L) ---------- */

const PNL_FILE: &str = "finanse.zarzadca.json";

fn pnl_dir(st: &State<'_, AppState>) -> Res<PathBuf> {
    st.cfg.lock().unwrap().pnl.clone().map(PathBuf::from).ok_or_else(|| "nie wybrano folderu na dane finansowe".into())
}

/// Folder P&L nie może leżeć we wspólnym folderze danych (widzą go współpracownicy).
#[tauri::command]
fn cfg_set_pnl(app: AppHandle, st: State<'_, AppState>, path: String) -> Res<()> {
    let p = PathBuf::from(&path);
    fs::create_dir_all(&p).map_err(|e| format!("nie można utworzyć folderu: {e}"))?;
    let mut c = st.cfg.lock().unwrap();
    if let Some(root) = c.root.clone() {
        let (pc, rc) = (fs::canonicalize(&p).map_err(es)?, fs::canonicalize(&root).map_err(es)?);
        if pc.starts_with(&rc) {
            return Err("Ten folder leży we wspólnym folderze danych, który widzą współpracownicy — wybierz inny, prywatny folder.".into());
        }
    }
    let probe = p.join(".zarzadca-test");
    fs::write(&probe, b"ok").map_err(|e| format!("brak prawa zapisu w folderze: {e}"))?;
    let _ = fs::remove_file(probe);
    c.pnl = Some(path);
    save_cfg(&app, &c)
}

#[tauri::command]
fn pnl_read(st: State<'_, AppState>) -> Res<Option<String>> {
    match fs::read_to_string(pnl_dir(&st)?.join(PNL_FILE)) {
        Ok(t) => Ok(Some(t)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(es(e)),
    }
}

/// Zapis danych finansowych + kopia z danego dnia w podfolderze „kopie”.
#[tauri::command]
fn pnl_write(st: State<'_, AppState>, text: String, day: String) -> Res<()> {
    let dir = pnl_dir(&st)?;
    write_atomic(&dir.join(PNL_FILE), text.as_bytes())?;
    let valid_day = day.len() == 10 && day.chars().enumerate().all(|(i, ch)| if i == 4 || i == 7 { ch == '-' } else { ch.is_ascii_digit() });
    if valid_day {
        let bk = dir.join("kopie").join(format!("finanse-{day}.json"));
        if !bk.exists() {
            let _ = write_atomic(&bk, text.as_bytes());
        }
    }
    Ok(())
}

#[tauri::command]
fn cfg_set(app: AppHandle, st: State<'_, AppState>, root: Option<String>, user: Option<String>, migrate: bool) -> Res<()> {
    let mut c = st.cfg.lock().unwrap();
    if let Some(r) = root {
        let new_root = PathBuf::from(&r);
        fs::create_dir_all(&new_root).map_err(|e| format!("nie można utworzyć folderu: {e}"))?;
        // sprawdź, czy da się tu pisać
        let probe = new_root.join(".zarzadca-test");
        fs::write(&probe, b"ok").map_err(|e| format!("brak prawa zapisu w folderze: {e}"))?;
        let _ = fs::remove_file(probe);
        if migrate {
            if let Some(old) = c.root.clone().map(PathBuf::from) {
                if old != new_root && !new_root.join("dane.json").exists() {
                    if old.join("dane.json").is_file() {
                        fs::copy(old.join("dane.json"), new_root.join("dane.json")).map_err(es)?;
                    }
                    if old.join("Skany").is_dir() {
                        copy_tree(&old.join("Skany"), &new_root.join("Skany")).map_err(es)?;
                    }
                }
            }
        }
        c.root = Some(r);
    }
    if let Some(u) = user {
        c.user = Some(u.trim().to_string());
    }
    save_cfg(&app, &c)
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for e in fs::read_dir(from)? {
        let e = e?;
        let dst = to.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_tree(&e.path(), &dst)?;
        } else if !dst.exists() {
            fs::copy(e.path(), dst)?;
        }
    }
    Ok(())
}

#[derive(Serialize)]
struct Probe {
    exists: bool,
    has_data: bool,
    saved_by: Option<String>,
    saved_at: Option<String>,
}

/// Co jest w wybranym folderze (przed jego użyciem) — np. dane współpracownika w udostępnionym folderze OneDrive.
#[tauri::command]
fn probe(path: String) -> Probe {
    let p = PathBuf::from(&path);
    let data = p.join("dane.json");
    let meta: Option<serde_json::Value> = fs::read_to_string(&data).ok().and_then(|t| serde_json::from_str(&t).ok());
    let get = |k: &str| meta.as_ref().and_then(|m| m.get(k)).and_then(|v| v.as_str()).map(String::from);
    Probe { exists: p.is_dir(), has_data: data.is_file(), saved_by: get("savedBy"), saved_at: get("savedAt") }
}

#[tauri::command]
async fn pick_folder(app: AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .set_title("Wybierz folder danych aplikacji Zarządca")
        .blocking_pick_folder()
        .and_then(|f| f.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
}

/* ---------- pliki w folderze danych (tylko wewnątrz tego folderu) ---------- */

fn resolve(st: &State<'_, AppState>, rel: &str) -> Res<PathBuf> {
    let root = st.cfg.lock().unwrap().root.clone().ok_or("nie wybrano folderu danych")?;
    let rel_path = Path::new(rel);
    if rel_path.is_absolute() || rel_path.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err("niedozwolona ścieżka".into());
    }
    Ok(PathBuf::from(root).join(rel_path))
}

/// Zapis „wszystko albo nic”: najpierw plik tymczasowy, potem podmiana — przerwany zapis nie psuje danych.
fn write_atomic(path: &Path, data: &[u8]) -> Res<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(es)?;
    }
    let name = path.file_name().ok_or("zła nazwa pliku")?.to_string_lossy().into_owned();
    let tmp = path.with_file_name(format!(".{name}.{}.tmp", std::process::id()));
    fs::write(&tmp, data).map_err(es)?;
    let mut last = None;
    for _ in 0..8 {
        // OneDrive / antywirus potrafią na chwilę zablokować plik — próbujemy ponownie
        match fs::rename(&tmp, path) {
            Ok(_) => return Ok(()),
            Err(e) => {
                last = Some(e);
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
        }
    }
    let _ = fs::remove_file(&tmp);
    Err(last.map(es).unwrap_or_else(|| "zapis nieudany".into()))
}

fn mtime_ms(m: &fs::Metadata) -> f64 {
    m.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_millis() as f64).unwrap_or(0.0)
}

#[tauri::command]
fn f_read(st: State<'_, AppState>, path: String) -> Res<Response> {
    match fs::read(resolve(&st, &path)?) {
        Ok(b) => Ok(Response::new(b)),
        Err(e) if e.kind() == ErrorKind::NotFound => Err("NotFound".into()),
        Err(e) => Err(es(e)),
    }
}

#[tauri::command]
fn f_read_text(st: State<'_, AppState>, path: String) -> Res<Option<String>> {
    match fs::read(resolve(&st, &path)?) {
        Ok(b) => Ok(Some(String::from_utf8_lossy(&b).into_owned())),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(es(e)),
    }
}

#[tauri::command]
fn f_write(st: State<'_, AppState>, request: Request<'_>) -> Res<()> {
    let raw = request.headers().get("x-path").and_then(|v| v.to_str().ok()).ok_or("brak ścieżki")?;
    let rel = percent_encoding::percent_decode_str(raw).decode_utf8().map_err(es)?.into_owned();
    let InvokeBody::Raw(data) = request.body() else {
        return Err("oczekiwano danych binarnych".into());
    };
    write_atomic(&resolve(&st, &rel)?, data)
}

#[tauri::command]
fn f_write_text(st: State<'_, AppState>, path: String, text: String) -> Res<()> {
    write_atomic(&resolve(&st, &path)?, text.as_bytes())
}

#[derive(Serialize)]
struct Stat {
    size: u64,
    mtime: f64,
    dir: bool,
}

#[tauri::command]
fn f_stat(st: State<'_, AppState>, path: String) -> Res<Option<Stat>> {
    match fs::metadata(resolve(&st, &path)?) {
        Ok(m) => Ok(Some(Stat { size: m.len(), mtime: mtime_ms(&m), dir: m.is_dir() })),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(es(e)),
    }
}

#[derive(Serialize)]
struct Entry {
    name: String,
    dir: bool,
    size: u64,
    mtime: f64,
}

#[tauri::command]
fn f_list(st: State<'_, AppState>, path: String) -> Res<Vec<Entry>> {
    let dir = resolve(&st, &path)?;
    let rd = match fs::read_dir(&dir) {
        Ok(r) => r,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(es(e)),
    };
    let mut out = vec![];
    for e in rd.flatten() {
        if let Ok(m) = e.metadata() {
            out.push(Entry { name: e.file_name().to_string_lossy().into_owned(), dir: m.is_dir(), size: m.len(), mtime: mtime_ms(&m) });
        }
    }
    Ok(out)
}

#[tauri::command]
fn f_mkdir(st: State<'_, AppState>, path: String) -> Res<()> {
    fs::create_dir_all(resolve(&st, &path)?).map_err(es)
}

/// Usuwa plik albo pusty folder (nigdy całych drzew folderów).
#[tauri::command]
fn f_remove(st: State<'_, AppState>, path: String) -> Res<()> {
    let p = resolve(&st, &path)?;
    let r = if p.is_dir() { fs::remove_dir(&p) } else { fs::remove_file(&p) };
    match r {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(es(e)),
    }
}

#[tauri::command]
fn f_rename(st: State<'_, AppState>, from: String, to: String) -> Res<()> {
    let (a, b) = (resolve(&st, &from)?, resolve(&st, &to)?);
    if let Some(d) = b.parent() {
        fs::create_dir_all(d).map_err(es)?;
    }
    fs::rename(a, b).map_err(es)
}

#[tauri::command]
fn f_copy(st: State<'_, AppState>, from: String, to: String) -> Res<()> {
    let (a, b) = (resolve(&st, &from)?, resolve(&st, &to)?);
    let data = fs::read(a).map_err(es)?;
    write_atomic(&b, &data)
}

/// „Pokaż w Eksploratorze”: folder otwiera, plik zaznacza.
#[tauri::command]
fn reveal(st: State<'_, AppState>, path: String) -> Res<()> {
    let p = resolve(&st, &path)?;
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let arg = if p.is_dir() { p.as_os_str().to_owned() } else {
            let mut s = std::ffi::OsString::from("/select,\"");
            s.push(p.as_os_str());
            s.push("\"");
            s
        };
        std::process::Command::new("explorer").raw_arg(arg).spawn().map_err(es)?;
    }
    #[cfg(target_os = "macos")]
    {
        let mut cmd = std::process::Command::new("open");
        if !p.is_dir() { cmd.arg("-R"); }
        cmd.arg(&p).spawn().map_err(es)?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let target = if p.is_dir() { p.clone() } else { p.parent().map(Path::to_path_buf).unwrap_or(p.clone()) };
        std::process::Command::new("xdg-open").arg(target).spawn().map_err(es)?;
    }
    Ok(())
}

/* ---------- aktualizacje ---------- */

#[derive(Serialize)]
struct UpdInfo {
    version: String,
    notes: Option<String>,
    date: Option<String>,
}

#[tauri::command]
async fn upd_check(app: AppHandle, st: State<'_, AppState>) -> Res<Option<UpdInfo>> {
    let upd = app.updater().map_err(es)?.check().await.map_err(es)?;
    Ok(upd.map(|u| {
        let info = UpdInfo { version: u.version.clone(), notes: u.body.clone(), date: u.date.map(|d| d.to_string()) };
        *st.pending.lock().unwrap() = Some(u);
        info
    }))
}

#[tauri::command]
async fn upd_install(app: AppHandle, st: State<'_, AppState>) -> Res<()> {
    let upd = st.pending.lock().unwrap().take().ok_or("najpierw sprawdź aktualizacje")?;
    let emitter = app.clone();
    let mut got: usize = 0;
    upd.download_and_install(
        move |chunk, total| {
            got += chunk;
            let _ = emitter.emit("upd-progress", (got, total));
        },
        || {},
    )
    .await
    .map_err(es)?;
    app.restart();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // drugie uruchomienie (np. podwójne kliknięcie ikony) tylko pokazuje otwarte okno
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let cfg = load_cfg(app.handle());
            app.manage(AppState { cfg: Mutex::new(cfg), pending: Mutex::new(None) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            cfg_get, cfg_set, probe, pick_folder, f_read, f_read_text, f_write, f_write_text, f_stat, f_list, f_mkdir, f_remove,
            f_rename, f_copy, reveal, upd_check, upd_install, cfg_set_pnl, pnl_read, pnl_write
        ])
        .run(tauri::generate_context!())
        .expect("nie udało się uruchomić aplikacji Zarządca");
}
