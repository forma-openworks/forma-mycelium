//! Настройки и папки программы, как у JetBrains: настройки — в `%APPDATA%\Forma\Mycelium`,
//! кеши, индексы и логи — в `%LOCALAPPDATA%\Forma\Mycelium`.
//!
//! Настройки разбиты на компоненты платформы Forma (внешний вид, раскладка клавиш) и
//! компоненты Mycelium (Discord, недавние проекты). Каждый лежит в своём файле
//! `options/<id>.json`. Старый общий `settings.json` при первом запуске переезжает туда сам.

use std::path::{Path, PathBuf};

use forma_actions::Keymap;
use forma_settings::{AppInfo, Component, Store};
use forma_ui::appearance::Appearance;
use forma_ui::fonts::Fonts;
use forma_ui::theme::Accent;
use serde::{Deserialize, Serialize};

pub const APP: AppInfo = AppInfo { id: "mycelium", dir_name: "Mycelium", title: "Forma Mycelium", version: env!("CARGO_PKG_VERSION") };

/// Папка настроек: `%APPDATA%\Forma\Mycelium`.
pub fn config_dir() -> Option<PathBuf> {
    APP.config_dir()
}

/// Кеши, индексы, языковые серверы: `%LOCALAPPDATA%\Forma\Mycelium`.
pub fn cache_dir() -> Option<PathBuf> {
    APP.cache_dir()
}

/// Пользовательские переводы: `%APPDATA%\Forma\Mycelium\locales\<язык>\*.ftl`.
pub fn user_locales_dir() -> Option<PathBuf> {
    config_dir().map(|d| d.join("locales"))
}

/// Свои темы: `%APPDATA%\Forma\Mycelium\themes\*.toml`.
pub fn user_themes_dir() -> Option<PathBuf> {
    config_dir().map(|d| d.join("themes"))
}

/// Активность в Discord.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Discord {
    /// Показывать в Discord, над каким модом идёт работа.
    pub enabled: bool,
}

impl Default for Discord {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl Component for Discord {
    const ID: &'static str = "discord";
}

/// Недавние проекты. Это память программы, а не настройка: в экспорт не попадает.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Recent {
    pub projects: Vec<PathBuf>,
    /// Проект, открытый при выходе: при запуске он открывается снова.
    pub last: Option<PathBuf>,
}

impl Component for Recent {
    const ID: &'static str = "recent-projects";
    const EXPORTABLE: bool = false;
}

impl Recent {
    pub fn remember(&mut self, path: &Path) {
        self.projects.retain(|p| p != path);
        self.projects.insert(0, path.to_path_buf());
        self.projects.truncate(10);
        self.last = Some(path.to_path_buf());
    }
}

/// Все настройки программы.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Settings {
    pub appearance: Appearance,
    pub fonts: Fonts,
    pub keymap: Keymap,
    pub discord: Discord,
    pub recent: Recent,
}

impl Settings {
    pub fn load(store: &mut Store) -> Self {
        if !store.initialized()
            && let Some(old) = migrate_legacy()
        {
            old.save(store);
            return old;
        }
        let had_fonts = store.exists::<Fonts>();
        let mut s = Self { appearance: store.load(), fonts: store.load(), keymap: store.load(), discord: store.load(), recent: store.load() };
        // Размер текста первых версий (основа шкалы, текст был на 10% мельче) переезжает в «Шрифты».
        if let Some(old) = s.appearance.legacy_font_size.take()
            && !had_fonts
        {
            s.fonts.ui.size = (old * 0.9 * 2.0).round() / 2.0;
        }
        s
    }

    /// Записывает изменившиеся компоненты.
    pub fn save(&self, store: &mut Store) {
        store.save(&self.appearance);
        store.save(&self.fonts);
        store.save(&self.keymap);
        store.save(&self.discord);
        store.save(&self.recent);
    }
}

/// Общий `settings.json` первых версий.
#[derive(Deserialize)]
#[serde(default)]
struct Legacy {
    language: Option<String>,
    theme: String,
    accent: Accent,
    font_size: f32,
    zoom: f32,
    recent_projects: Vec<PathBuf>,
    last_project: Option<PathBuf>,
    discord_presence: bool,
}

impl Default for Legacy {
    fn default() -> Self {
        let a = Appearance::default();
        Self {
            language: a.language,
            theme: "Dark".to_owned(),
            accent: Accent::Violet,
            font_size: 16.0,
            zoom: a.zoom,
            recent_projects: Vec::new(),
            last_project: None,
            discord_presence: true,
        }
    }
}

impl Legacy {
    fn into_settings(self) -> Settings {
        let theme = match self.theme.as_str() {
            "Light" => "mycelium-light",
            _ => "mycelium-dark",
        };
        let mut fonts = Fonts::default();
        fonts.ui.size = (self.font_size * 0.9 * 2.0).round() / 2.0;
        Settings {
            appearance: Appearance { language: self.language, theme: theme.to_owned(), accent: Some(self.accent), zoom: self.zoom, ..Appearance::default() },
            fonts,
            keymap: Keymap::default(),
            discord: Discord { enabled: self.discord_presence },
            recent: Recent { projects: self.recent_projects, last: self.last_project },
        }
    }
}

/// Переносит старый `settings.json` в компоненты. Сам файл переименовывается
/// в `settings.json.migrated`, чтобы было видно, куда делись настройки.
fn migrate_legacy() -> Option<Settings> {
    let path = config_dir()?.join("settings.json");
    let old: Legacy = read_json(&path)?;
    let _ = std::fs::rename(&path, path.with_extension("json.migrated"));
    log::info!("settings.json migrated to options/");
    Some(old.into_settings())
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
    let result = serde_json::to_string_pretty(value).map_err(std::io::Error::other).and_then(|json| forma_settings::write_atomic(path, json.as_bytes()));
    if let Err(err) = result {
        log::warn!("Failed to save {}: {err}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_file_maps_to_components() {
        let json = r#"{"language":"ru","theme":"Light","accent":"Teal","font_size":15.0,"zoom":1.25,"recent_projects":["D:/mods/a"],"last_project":"D:/mods/a","discord_presence":false}"#;
        let s = serde_json::from_str::<Legacy>(json).unwrap().into_settings();
        assert_eq!(s.appearance.language.as_deref(), Some("ru"));
        assert_eq!(s.appearance.theme, "mycelium-light");
        assert_eq!(s.appearance.accent, Some(Accent::Teal));
        assert_eq!(s.fonts.ui.size, 13.5);
        assert_eq!(s.appearance.zoom, 1.25);
        assert!(!s.discord.enabled);
        assert_eq!(s.recent.projects, vec![PathBuf::from("D:/mods/a")]);
        assert!(s.keymap.overrides.is_empty());
    }
}
