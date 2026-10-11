//! Папка `.mycelium` в корне проекта — точка инициализации проекта Forma Mycelium,
//! как `.idea` у IDEA. В ней лежит всё локальное: описание проекта, раскладка интерфейса,
//! состояние дерева файлов, позже — карта мода, история исключений и кеши.
//!
//! Если папки нет, она создаётся с исходными параметрами при первом открытии.
//! В Windows папка получает атрибут «скрытый», как `.git`.

use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Имя папки проекта.
pub const DIR: &str = ".mycelium";
/// Описание проекта.
pub const PROJECT_FILE: &str = "project.json";
/// Личная раскладка и состояние интерфейса (в git не попадает).
pub const WORKSPACE_FILE: &str = "workspace.json";
/// Версия формата `project.json`.
pub const FORMAT_VERSION: u32 = 1;

/// Что в `.mycelium` личное и не должно попадать в git: раскладка и кеши.
/// Общее (описание проекта, позже карта мода со стикерами) остаётся в репозитории.
const GITIGNORE: &str = "# Личное для каждого разработчика: раскладка интерфейса и кеши.\nworkspace.json\ncache/\n";

/// `project.json`: описание проекта.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectMeta {
    pub version: u32,
    /// Отображаемое имя проекта.
    pub name: String,
    /// Когда проект впервые открыли в Forma Mycelium (секунды Unix).
    pub created: u64,
}

impl Default for ProjectMeta {
    fn default() -> Self {
        Self { version: FORMAT_VERSION, name: String::new(), created: 0 }
    }
}

/// Путь к папке `.mycelium` проекта.
pub fn dir(root: &Path) -> PathBuf {
    root.join(DIR)
}

/// Есть ли у папки точка инициализации.
pub fn is_initialized(root: &Path) -> bool {
    dir(root).join(PROJECT_FILE).is_file()
}

/// Создаёт `.mycelium` с исходными параметрами (если её ещё нет) и возвращает описание проекта.
pub fn open_or_init(root: &Path) -> io::Result<(ProjectMeta, bool)> {
    if !root.is_dir() {
        return Err(io::Error::new(io::ErrorKind::NotFound, root.display().to_string()));
    }
    let dir = dir(root);
    let existed = is_initialized(root);
    std::fs::create_dir_all(&dir)?;
    set_hidden(&dir);
    if !dir.join(".gitignore").exists() {
        std::fs::write(dir.join(".gitignore"), GITIGNORE)?;
    }
    let meta = match read_json::<ProjectMeta>(root, PROJECT_FILE) {
        Some(meta) if existed => meta,
        _ => {
            let meta = ProjectMeta { version: FORMAT_VERSION, name: default_name(root), created: now() };
            write_json(root, PROJECT_FILE, &meta)?;
            meta
        }
    };
    Ok((meta, !existed))
}

fn default_name(root: &Path) -> String {
    root.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| root.display().to_string())
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Читает JSON-файл из `.mycelium`. Испорченный файл не роняет программу — просто не читается.
pub fn read_json<T: for<'de> Deserialize<'de>>(root: &Path, name: &str) -> Option<T> {
    let path = dir(root).join(name);
    let text = std::fs::read_to_string(&path).ok()?;
    match serde_json::from_str(&text) {
        Ok(v) => Some(v),
        Err(err) => {
            log::warn!("Failed to parse {}: {err}", path.display());
            None
        }
    }
}

/// Пишет JSON-файл в `.mycelium` через временный файл, чтобы при сбое не остался обрезок.
pub fn write_json<T: Serialize>(root: &Path, name: &str, value: &T) -> io::Result<()> {
    let dir = dir(root);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(name);
    let json = serde_json::to_string_pretty(value).map_err(io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, &path)
}

/// Скрыт ли файл или папка: имя с точки или атрибут «скрытый» в Windows.
pub fn is_hidden(path: &Path) -> bool {
    let dot = path.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.'));
    dot || has_hidden_attribute(path)
}

#[cfg(windows)]
fn has_hidden_attribute(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    const HIDDEN: u32 = 0x2;
    std::fs::symlink_metadata(path).is_ok_and(|m| m.file_attributes() & HIDDEN != 0)
}

#[cfg(not(windows))]
fn has_hidden_attribute(_path: &Path) -> bool {
    false
}

/// Ставит атрибут «скрытый» (в Windows). Если не вышло — не беда, папка просто видна.
#[cfg(windows)]
pub fn set_hidden(path: &Path) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_HIDDEN, GetFileAttributesW, INVALID_FILE_ATTRIBUTES, SetFileAttributesW};
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    // SAFETY: строка заканчивается нулём и живёт до конца вызовов.
    unsafe {
        let attrs = GetFileAttributesW(wide.as_ptr());
        if attrs != INVALID_FILE_ATTRIBUTES && attrs & FILE_ATTRIBUTE_HIDDEN == 0 && SetFileAttributesW(wide.as_ptr(), attrs | FILE_ATTRIBUTE_HIDDEN) == 0 {
            log::warn!("Failed to hide {}", path.display());
        }
    }
}

#[cfg(not(windows))]
pub fn set_hidden(_path: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mycelium-store-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn first_open_creates_hidden_init_point() {
        let root = temp("init");
        assert!(!is_initialized(&root));
        let (meta, created) = open_or_init(&root).unwrap();
        assert!(created);
        assert!(is_initialized(&root));
        assert_eq!(meta.version, FORMAT_VERSION);
        assert!(meta.name.starts_with("mycelium-store-init"));
        assert!(dir(&root).join(".gitignore").is_file());
        assert!(is_hidden(&dir(&root)));
        #[cfg(windows)]
        assert!(has_hidden_attribute(&dir(&root)), "в Windows папка скрыта атрибутом");

        // Второе открытие ничего не пересоздаёт.
        let mut renamed = meta.clone();
        renamed.name = "Мой мод".into();
        write_json(&root, PROJECT_FILE, &renamed).unwrap();
        let (again, created) = open_or_init(&root).unwrap();
        assert!(!created);
        assert_eq!(again.name, "Мой мод");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn broken_project_file_is_recreated() {
        let root = temp("broken");
        std::fs::create_dir_all(dir(&root)).unwrap();
        std::fs::write(dir(&root).join(PROJECT_FILE), "{ не json").unwrap();
        let (meta, _) = open_or_init(&root).unwrap();
        assert_eq!(meta.version, FORMAT_VERSION);
        assert!(read_json::<ProjectMeta>(&root, PROJECT_FILE).is_some(), "файл переписан исправным");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_folder_is_an_error() {
        assert!(open_or_init(Path::new("Z:/нет/такой/папки")).is_err());
    }
}
