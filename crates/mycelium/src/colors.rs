//! Цвета Forma Mycelium поверх цветов платформы: происхождение кода, уровни лога,
//! состояния запусков, значки редакторов и файлов. Их, как и всё остальное, задаёт тема.

use std::sync::{Arc, OnceLock, RwLock};

/// Встроенные темы — Mycelium Dark и Light из платформы: тёмная по умолчанию идёт первой.
/// Остальные темы — плагины.
pub const BUILTIN_THEMES: &[&str] = &forma_ui::theme::MYCELIUM_THEMES;

/// Тема, если в настройках ничего не выбрано.
pub const DEFAULT_THEME: &str = "mycelium-dark";

forma_ui::theme_tokens! {
    /// Цвета Mycelium. Значения по умолчанию — для тёмной темы.
    pub struct ModColors {
        /// Код и ресурсы своего мода.
        own = "origin.own" => "$accent",
        /// Minecraft.
        minecraft = "origin.minecraft" => "#6abe30",
        /// NeoForge.
        neoforge = "origin.neoforge" => "#f08a24",
        /// Fabric.
        fabric = "origin.fabric" => "#dbc6a0",
        /// Forge.
        forge = "origin.forge" => "#608cc8",
        /// Другие моды.
        other_mod = "origin.other-mod" => "#e879f9",
        /// Плашка своего мода (салатовая): у чужих модов плашки цвета «другие моды».
        tag_own = "mod-tag.own" => "#a3e635",
        /// Обычные библиотеки.
        library = "origin.library" => "#94a3b8",
        /// JDK.
        java = "origin.java" => "#be8c5a",
        /// Происхождение неизвестно.
        unknown = "origin.unknown" => "$text.faint",

        /// Ошибки и краши в логе.
        level_error = "level.error" => "$danger",
        /// Предупреждения в логе.
        level_warn = "level.warn" => "$warning",
        /// Обычные сообщения лога.
        level_info = "level.info" => "$text.muted",
        /// Отладочные сообщения лога.
        level_debug = "level.debug" => "$text.faint",

        /// Идёт запуск.
        run_running = "run.running" => "$accent",
        /// Запуск завершился успешно; кнопка «Запустить».
        run_ok = "run.ok" => "$success",
        /// Запуск упал; кнопка «Остановить».
        run_failed = "run.failed" => "$danger",
        /// Запуск остановлен.
        run_stopped = "run.stopped" => "$text.muted",

        /// Значок редактора «Проект».
        kind_project = "editor-kind.project" => "$accent",
        /// Значок редактора кода.
        kind_code = "editor-kind.code" => "#60a5fa",
        /// Значок карты мода.
        kind_map = "editor-kind.map" => "#c084fc",
        /// Значок окна исключений.
        kind_exceptions = "editor-kind.exceptions" => "$warning",
        /// Значок консоли.
        kind_console = "editor-kind.console" => "#2dd4bf",
        /// Значок превью модели.
        kind_model = "editor-kind.model" => "#fb923c",
        /// Значок модов запуска.
        kind_mods = "editor-kind.mods" => "$origin.other-mod",

        /// Файлы Java.
        file_java = "file.java" => "#e8833a",
        /// Файлы Kotlin.
        file_kotlin = "file.kotlin" => "#a78bfa",
        /// JSON и mcmeta.
        file_json = "file.json" => "#facc15",
        /// Картинки.
        file_image = "file.image" => "#34d399",
        /// Модели OBJ.
        file_model = "file.model" => "#fb923c",
    }
}

fn slot() -> &'static RwLock<Arc<ModColors>> {
    static CURRENT: OnceLock<RwLock<Arc<ModColors>>> = OnceLock::new();
    CURRENT.get_or_init(|| {
        let themes = forma_ui::theme::Themes::new(BUILTIN_THEMES, None);
        RwLock::new(Arc::new(themes.resolve(DEFAULT_THEME, DEFAULT_THEME).group(ModColors::TOKENS, ModColors::resolve)))
    })
}

/// Цвета Mycelium текущей темы.
pub fn current() -> Arc<ModColors> {
    slot().read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Новые цвета после смены темы.
pub fn set(colors: ModColors) {
    *slot().write().unwrap_or_else(|e| e.into_inner()) = Arc::new(colors);
}

#[cfg(test)]
mod tests {
    use super::*;
    use forma_ui::theme::Themes;

    #[test]
    fn builtin_themes_resolve_cleanly() {
        // Темы из плагинов программы проверяются вместе со встроенными.
        let plugin_themes: Vec<&str> = crate::plugins::catalog()
            .into_iter()
            .filter_map(|e| match e.source {
                forma_plugins::Source::Bundled { files } => Some(files.iter().filter(|(p, _)| p.ends_with(".toml")).map(|(_, text)| *text).collect::<Vec<_>>()),
                _ => None,
            })
            .flatten()
            .collect();
        let all: Vec<&str> = BUILTIN_THEMES.iter().copied().chain(plugin_themes).collect();
        let themes = Themes::new(&all, None);
        let ids: Vec<&str> = themes.list().iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["mycelium-dark", "mycelium-light", "vs-dark"]);
        for id in ids {
            let t = themes.resolve(id, DEFAULT_THEME);
            assert!(t.errors.is_empty(), "{id}: {:?}", t.errors);
            let _ = t.group(ModColors::TOKENS, ModColors::resolve);
            for (class, _) in crate::highlight::CLASS_SELECTORS {
                assert!(t.syntax.contains_key(*class), "{id}: нет подсветки «{class}»");
            }
        }
        let names: Vec<String> = themes.list().iter().map(|t| t.name.clone()).collect();
        assert_eq!(names, vec!["Mycelium Dark", "Mycelium Light", "Visual Studio Dark"]);
    }
}
