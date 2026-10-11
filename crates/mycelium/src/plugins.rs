//! Каталог плагинов Forma Mycelium: что можно поставить одной кнопкой.
//!
//! - jdtls — подсказки Java (ошибки, автодополнение, переход к определению);
//! - kotlin-lsp — то же для Kotlin (появится позже);
//! - Python — скрипты в духе Blender (появится вместе с плагином Python);
//! - Visual Studio Dark — тема, которую программа везёт в себе и ставит без сети.
//!
//! Плагины лежат в `%LOCALAPPDATA%\Forma\Mycelium\plugins`. jdtls первых версий стоял в
//! `servers\jdtls` — при запуске он переносится в плагины, скачивать заново не нужно.

use std::path::PathBuf;

use forma_i18n::t;
use forma_plugins::{CatalogEntry, Kind, PluginSettings, Plugins, Source};

use crate::lsp::install;

pub const JDTLS: &str = "jdtls";
pub const KOTLIN_LSP: &str = "kotlin-lsp";
pub const PYTHON: &str = "python";
pub const VS_DARK: &str = "visual-studio-dark";
/// id темы из плагина Visual Studio Dark (до плагинов она была встроенной).
const VS_DARK_THEME: &str = "vs-dark";

/// Папка плагинов.
pub fn root() -> Option<PathBuf> {
    crate::settings::cache_dir().map(|d| d.join("plugins"))
}

/// Что можно поставить. Названия и описания — на языке интерфейса.
pub fn catalog() -> Vec<CatalogEntry> {
    vec![
        CatalogEntry {
            id: JDTLS,
            name: t!("plugin-jdtls-name"),
            description: t!("plugin-jdtls-about"),
            kind: Kind::Tool,
            size: Some(51_500_000),
            author: "Eclipse Foundation",
            license: "EPL-2.0",
            homepage: "https://github.com/eclipse-jdtls/eclipse.jdt.ls",
            icon: "jdk",
            source: Source::Download { url: install::JDTLS_URL },
            recommended: true,
            validate: Some(install::validate),
            themes: None,
        },
        CatalogEntry {
            id: KOTLIN_LSP,
            name: t!("plugin-kotlin-name"),
            description: t!("plugin-kotlin-about"),
            kind: Kind::Tool,
            size: None,
            author: "JetBrains",
            license: "Apache-2.0",
            homepage: "https://github.com/Kotlin/kotlin-lsp",
            icon: "file-kotlin",
            source: Source::Later,
            recommended: true,
            validate: None,
            themes: None,
        },
        CatalogEntry {
            id: PYTHON,
            name: t!("plugin-python-name"),
            description: t!("plugin-python-about"),
            kind: Kind::Script,
            size: None,
            author: "Python Software Foundation",
            license: "PSF-2.0",
            homepage: "https://www.python.org",
            icon: "file-code",
            source: Source::Later,
            recommended: true,
            validate: None,
            themes: None,
        },
        CatalogEntry {
            id: VS_DARK,
            name: t!("plugin-vs-dark-name"),
            description: t!("plugin-vs-dark-about"),
            kind: Kind::Themes,
            size: None,
            author: "FORMA",
            license: "MIT",
            homepage: "",
            icon: "palette",
            source: Source::Bundled { files: &[("themes/vs-dark.toml", include_str!("../plugins/visual-studio-dark/themes/vs-dark.toml"))] },
            // Тема — не первая необходимость: на экране приветствия её не предлагаем.
            recommended: false,
            validate: None,
            themes: Some("themes"),
        },
    ]
}

pub fn entry(id: &str) -> Option<CatalogEntry> {
    catalog().into_iter().find(|e| e.id == id)
}

/// Плагины при запуске: jdtls из старого места переезжает в плагины. `theme` — тема из
/// настроек: Visual Studio Dark раньше была встроенной, и тем, кто её выбрал, плагин
/// ставится сам, чтобы тема не пропала.
pub fn open(theme: &str) -> Plugins {
    let mut plugins = Plugins::new(root());
    if theme == VS_DARK_THEME
        && plugins.get(VS_DARK).is_none()
        && let Some(vs) = entry(VS_DARK)
    {
        plugins.install(&vs);
        plugins.take_events();
    }
    if let (Some(old), Some(jdtls)) = (crate::settings::cache_dir().map(|d| d.join("servers").join(JDTLS)), entry(JDTLS)) {
        match plugins.adopt(&jdtls, &old) {
            Ok(true) => log::info!("jdtls moved from {} to plugins", old.display()),
            Ok(false) => {}
            Err(e) => log::warn!("jdtls in {} was not moved to plugins: {e}", old.display()),
        }
    }
    plugins
}

/// Папка плагина, если он установлен и включён.
pub fn active_dir(plugins: &Plugins, settings: &PluginSettings, id: &str) -> Option<PathBuf> {
    plugins.get(id).filter(|_| settings.enabled(id)).map(|p| p.dir.clone())
}

/// Папки тем из установленных и включённых плагинов.
pub fn theme_dirs(plugins: &Plugins, settings: &PluginSettings) -> Vec<PathBuf> {
    plugins.installed().iter().filter(|p| settings.enabled(p.id())).filter_map(|p| p.themes_dir()).collect()
}
