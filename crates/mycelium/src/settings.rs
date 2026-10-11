//! Настройки и папки программы, как у JetBrains:
//! настройки — в `%APPDATA%\Forma\Mycelium`, кеши, индексы и логи — в `%LOCALAPPDATA%\Forma\Mycelium`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::theme::{Accent, ThemeKind};

/// Папка настроек: `%APPDATA%\Forma\Mycelium`.
pub fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("Forma").join("Mycelium"))
}

/// Пользовательские переводы: `%APPDATA%\Forma\Mycelium\locales\<язык>\*.ftl`.
pub fn user_locales_dir() -> Option<PathBuf> {
    config_dir().map(|d| d.join("locales"))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Язык интерфейса; `None` — как в системе.
    pub language: Option<String>,
    pub theme: ThemeKind,
    pub accent: Accent,
    pub font_size: f32,
    pub zoom: f32,
    pub recent_projects: Vec<PathBuf>,
    /// Проект, открытый при выходе: при запуске он открывается снова.
    pub last_project: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: None,
            theme: ThemeKind::Dark,
            accent: Accent::Violet,
            font_size: 16.0,
            zoom: 1.0,
            recent_projects: Vec::new(),
            last_project: None,
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        config_dir().and_then(|d| read_json(&d.join("settings.json"))).unwrap_or_default()
    }

    pub fn save(&self) {
        if let Some(dir) = config_dir() {
            write_json(&dir.join("settings.json"), self);
        }
    }

    pub fn remember_project(&mut self, path: &Path) {
        self.recent_projects.retain(|p| p != path);
        self.recent_projects.insert(0, path.to_path_buf());
        self.recent_projects.truncate(10);
        self.last_project = Some(path.to_path_buf());
    }

    /// Код языка интерфейса с учётом «как в системе».
    pub fn effective_language(&self) -> String {
        self.language.clone().unwrap_or_else(mycelium_i18n::system_language)
    }
}

pub fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    let text = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str(&text) {
        Ok(v) => Some(v),
        Err(err) => {
            log::warn!("Failed to parse {}: {err}", path.display());
            None
        }
    }
}

/// Пишет JSON через временный файл, чтобы при сбое не остался обрезанный файл.
pub fn write_json<T: Serialize>(path: &Path, value: &T) {
    let result = (|| -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_string_pretty(value).map_err(std::io::Error::other)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, path)
    })();
    if let Err(err) = result {
        log::warn!("Failed to save {}: {err}", path.display());
    }
}
