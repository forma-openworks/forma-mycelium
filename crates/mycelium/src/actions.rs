//! Действия Forma Mycelium: id, группы, названия и сочетания по умолчанию.
//! Сочетания по умолчанию — как в IntelliJ IDEA для Windows, чтобы руки не переучивать.

use forma_actions::{Action, Context, Registry};

pub const OPEN_PROJECT: &str = "file.openProject";
pub const CLOSE_PROJECT: &str = "file.closeProject";
pub const SAVE_ALL: &str = "file.saveAll";
pub const SETTINGS: &str = "file.settings";
pub const EXPORT_SETTINGS: &str = "file.exportSettings";
pub const IMPORT_SETTINGS: &str = "file.importSettings";
pub const QUIT: &str = "file.quit";

pub const SYNC_GRADLE: &str = "project.syncGradle";

pub const RUN: &str = "run.start";
pub const STOP: &str = "run.stop";
pub const STOP_ALL: &str = "run.stopAll";
pub const EDIT_RUN_CONFIGS: &str = "run.editConfigurations";

pub const ZOOM_IN: &str = "view.zoomIn";
pub const ZOOM_OUT: &str = "view.zoomOut";
pub const ZOOM_RESET: &str = "view.zoomReset";
pub const QUICK_THEME: &str = "view.quickSwitchTheme";

pub const SHOW_PROJECT: &str = "window.project";
pub const SHOW_CONSOLE: &str = "window.console";
pub const SHOW_EXCEPTIONS: &str = "window.exceptions";
pub const MAXIMIZE: &str = "window.maximizeArea";
pub const DETACH: &str = "window.detachTab";
pub const RETURN_WINDOWS: &str = "window.returnAll";

pub const COMPLETION: &str = "editor.completion";
pub const GOTO_DEFINITION: &str = "editor.gotoDefinition";

pub const FIND_ACTION: &str = "help.findAction";
pub const ABOUT: &str = "help.about";

pub fn registry() -> Registry {
    use Context::{Editor, Global};
    let mut r = Registry::default();
    let mut add = |id, group, title, context, keys: &[&str]| r.register(Action::new(id, group, title, context, keys));

    add(OPEN_PROJECT, "action-group-file", "action-open-project", Global, &[]);
    add(CLOSE_PROJECT, "action-group-file", "action-close-project", Global, &[]);
    add(SAVE_ALL, "action-group-file", "action-save-all", Global, &["Ctrl+S"]);
    add(SETTINGS, "action-group-file", "action-settings", Global, &["Ctrl+Alt+S"]);
    add(EXPORT_SETTINGS, "action-group-file", "action-export-settings", Global, &[]);
    add(IMPORT_SETTINGS, "action-group-file", "action-import-settings", Global, &[]);
    add(QUIT, "action-group-file", "action-quit", Global, &[]);

    add(SYNC_GRADLE, "action-group-project", "action-sync-gradle", Global, &["Ctrl+Shift+O"]);

    add(RUN, "action-group-run", "action-run", Global, &["Shift+F10"]);
    add(STOP, "action-group-run", "run-stop", Global, &["Ctrl+F2"]);
    add(STOP_ALL, "action-group-run", "run-stop-all", Global, &[]);
    add(EDIT_RUN_CONFIGS, "action-group-run", "run-edit-configs", Global, &[]);

    add(ZOOM_IN, "action-group-view", "action-zoom-in", Global, &["Ctrl+=", "Ctrl++"]);
    add(ZOOM_OUT, "action-group-view", "action-zoom-out", Global, &["Ctrl+-"]);
    add(ZOOM_RESET, "action-group-view", "action-zoom-reset", Global, &["Ctrl+0"]);
    add(QUICK_THEME, "action-group-view", "action-quick-theme", Global, &["Ctrl+`"]);

    add(SHOW_PROJECT, "action-group-window", "action-show-project", Global, &["Alt+1"]);
    add(SHOW_CONSOLE, "action-group-window", "action-show-console", Global, &["Alt+4"]);
    add(SHOW_EXCEPTIONS, "action-group-window", "action-show-exceptions", Global, &["Alt+6"]);
    add(MAXIMIZE, "action-group-window", "dock-maximize", Context::Layout, &["Ctrl+Shift+F12"]);
    add(DETACH, "action-group-window", "dock-detach", Global, &[]);
    add(RETURN_WINDOWS, "action-group-window", "action-return-windows", Global, &[]);

    add(COMPLETION, "action-group-editor", "action-completion", Editor, &["Ctrl+Space"]);
    add(GOTO_DEFINITION, "action-group-editor", "action-goto-definition", Editor, &["F12", "Ctrl+B"]);

    add(FIND_ACTION, "action-group-help", "action-find-action", Global, &["Ctrl+Shift+A"]);
    add(ABOUT, "action-group-help", "action-about", Global, &[]);
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_shortcuts_do_not_clash() {
        let r = registry();
        let keymap = forma_actions::Keymap::default();
        for a in r.iter() {
            for key in keymap.shortcuts(&r, &a.id) {
                assert!(keymap.conflicts(&r, *key, &a.id).is_empty(), "{key} у {} занято ещё кем-то", a.id);
            }
        }
    }

    #[test]
    fn every_title_is_translated() {
        let r = registry();
        for lang in forma_i18n::builtin_languages() {
            for a in r.iter() {
                let forma_actions::Title::Key(key) = &a.title else { continue };
                assert!(forma_i18n::has_message(lang, key), "нет перевода {key} ({lang})");
                assert!(forma_i18n::has_message(lang, &a.group), "нет перевода {} ({lang})", a.group);
            }
        }
    }
}
