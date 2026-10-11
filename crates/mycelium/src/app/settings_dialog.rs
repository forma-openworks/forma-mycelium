//! Окно настроек, «Найти действие», экспорт и импорт настроек.
//!
//! Окно настроек работает с черновиком: изменения видны в нём сразу, а к программе
//! применяются кнопками «Применить» и «ОК». «Отмена» и Escape их отбрасывают.

use std::path::Path;

use eframe::egui::{self, Align, Layout, RichText, Ui, vec2};
use forma_actions::{Context as ActionContext, Keymap};
use forma_i18n::t;
use forma_settings::export::{self, Archive, Status};
use forma_settings::{Component, decode};
use forma_ui::appearance::{self, Appearance, PageContext, ThemeOption};
use forma_ui::font_page::{self, FontPage};
use forma_ui::fonts::Fonts;
use forma_ui::find_action::{FindAction, Item};
use forma_ui::keymap_page::{self, KeymapPage};
use forma_ui::plugins_page::{self, PluginsPage};
use forma_ui::theme_tools;
use forma_plugins::PluginSettings;
use forma_ui::settings_window::{self as sw, Footer, PageInfo, SettingsWindow};
use forma_ui::theme::{Origin, Palette};

use super::{APP_TITLE, Cmd, Modal, MyceliumApp};
use crate::actions;
use crate::settings::{APP, Discord, Settings};

const DISCORD_PAGE: &str = "discord";
/// Пункты «Найти действие», которые открывают страницу настроек.
const SETTINGS_ITEM: &str = "settings:";

pub(super) struct SettingsDialog {
    window: SettingsWindow,
    draft: Settings,
    keymap: KeymapPage,
    fonts: FontPage,
    plugins: PluginsPage,
    /// «Секретный проект» — настройка открытого проекта, а не программы.
    discord_secret: bool,
}

/// Компоненты, которые можно перенести экспортом: id, название, что внутри.
fn exportable() -> Vec<(&'static str, String, String)> {
    vec![
        (Appearance::ID, t!("settings-page-appearance"), t!("settings-component-appearance")),
        (Fonts::ID, t!("settings-page-fonts"), t!("settings-component-fonts")),
        (Keymap::ID, t!("settings-page-keymap"), t!("settings-component-keymap")),
        (PluginSettings::ID, t!("settings-page-plugins"), t!("settings-component-plugins")),
        (Discord::ID, t!("settings-page-discord"), t!("settings-component-discord")),
    ]
}

fn component_title(id: &str) -> String {
    exportable().into_iter().find(|(i, ..)| *i == id).map(|(_, title, _)| title).unwrap_or_else(|| id.to_owned())
}

pub(super) struct ExportDialog {
    checked: Vec<(&'static str, bool)>,
}

impl ExportDialog {
    pub(super) fn new() -> Self {
        Self { checked: exportable().into_iter().map(|(id, ..)| (id, true)).collect() }
    }
}

struct ImportItem {
    id: String,
    status: Status,
    checked: bool,
}

pub(super) struct ImportDialog {
    archive: Archive,
    items: Vec<ImportItem>,
    /// Плагины из архива, которых здесь нет: после импорта их предлагается поставить.
    missing_plugins: Vec<String>,
    /// Итог после импорта: что применилось и что пропущено.
    report: Option<Vec<(String, Status)>>,
}

impl MyceliumApp {
    pub(super) fn open_settings(&mut self, page: Option<String>) {
        let page = page.unwrap_or_else(|| self.settings_page.clone());
        self.modal = Some(Modal::Settings(Box::new(SettingsDialog {
            window: SettingsWindow::new(page),
            draft: self.settings.clone(),
            keymap: KeymapPage::default(),
            fonts: FontPage::default(),
            plugins: PluginsPage::default(),
            discord_secret: self.view.discord_secret,
        })));
    }

    fn settings_pages(&self) -> Vec<PageInfo> {
        let platform = t!("settings-section-platform");
        vec![
            PageInfo { id: appearance::PAGE.into(), section: platform.clone(), title: t!("settings-page-appearance"), keywords: appearance::keywords() },
            PageInfo { id: theme_tools::INSPECTOR_PAGE.into(), section: platform.clone(), title: t!("settings-page-theme-tokens"), keywords: theme_tools::keywords(&theme_kit()) },
            PageInfo { id: font_page::PAGE.into(), section: platform.clone(), title: t!("settings-page-fonts"), keywords: font_page::keywords() },
            PageInfo { id: keymap_page::PAGE.into(), section: platform.clone(), title: t!("settings-page-keymap"), keywords: keymap_page::keywords(&self.actions) },
            PageInfo { id: plugins_page::PAGE.into(), section: platform, title: t!("settings-page-plugins"), keywords: plugins_page::keywords() },
            PageInfo {
                id: DISCORD_PAGE.into(),
                section: APP_TITLE.into(),
                title: t!("settings-page-discord"),
                keywords: vec![t!("settings-discord"), t!("settings-discord-secret")],
            },
        ]
    }

    /// Применяет черновик настроек к программе и сохраняет его.
    fn apply_settings(&mut self, d: &SettingsDialog) {
        let language_changed = d.draft.appearance.effective_language() != self.settings.appearance.effective_language();
        self.settings.appearance = d.draft.appearance.clone();
        self.settings.fonts = d.draft.fonts.clone();
        self.settings.keymap = d.draft.keymap.clone();
        self.settings.discord = d.draft.discord.clone();
        // Список установленных плагинов ведёт сама программа — из черновика берём только выключенные.
        self.settings.plugins.disabled = d.draft.plugins.disabled.clone();
        self.view.discord_secret = d.discord_secret;
        if language_changed {
            forma_i18n::set_language(&self.settings.appearance.effective_language());
        }
        self.ctx.set_zoom_factor(self.settings.appearance.zoom);
        self.settings.save(&mut self.store);
    }

    fn draft_dirty(&self, d: &SettingsDialog) -> bool {
        d.draft.appearance != self.settings.appearance
            || d.draft.fonts != self.settings.fonts
            || d.draft.keymap != self.settings.keymap
            || d.draft.discord != self.settings.discord
            || d.draft.plugins.disabled != self.settings.plugins.disabled
            || d.discord_secret != self.view.discord_secret
    }

    /// Окно настроек. Возвращает `true`, когда его пора закрыть.
    pub(super) fn settings_ui(&mut self, ui: &mut Ui, pal: &Palette, d: &mut SettingsDialog) -> bool {
        let pages = self.settings_pages();
        let live = &self.settings;
        let secret = self.view.discord_secret;
        let modified = |id: &str| match id {
            appearance::PAGE => d.draft.appearance != live.appearance,
            font_page::PAGE => d.draft.fonts != live.fonts,
            keymap_page::PAGE => d.draft.keymap != live.keymap,
            plugins_page::PAGE => d.draft.plugins.disabled != live.plugins.disabled,
            DISCORD_PAGE => d.draft.discord != live.discord || d.discord_secret != secret,
            _ => false,
        };
        // Флаги страниц считаются до отрисовки: черновик меняется внутри страниц.
        let flags: Vec<(String, bool)> = pages.iter().map(|p| (p.id.clone(), modified(&p.id))).collect();
        let dirty = flags.iter().any(|(_, m)| *m);
        let modified = |id: &str| flags.iter().any(|(p, m)| p == id && *m);
        let max = ui.ctx().content_rect().size() - vec2(48.0, 48.0);
        let project_open = self.project.is_some();
        let themes: Vec<ThemeOption> = self.themes.list().iter().map(|t| ThemeOption { id: t.id.clone(), name: t.name.clone(), builtin: t.origin == Origin::Builtin }).collect();
        let mut theme_errors = self.theme.errors.clone();
        theme_errors.extend(self.themes.broken.iter().map(|(path, e)| format!("{}: {e}", path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())));
        let cx = PageContext {
            themes_dir: self.themes.dir(),
            themes: &themes,
            default_theme: crate::colors::DEFAULT_THEME,
            theme_errors: &theme_errors,
        };
        // Шрифты темы, выбранной в черновике: от неё зависит, какие поля заблокированы.
        let draft_theme_fonts = self.themes.resolve(&d.draft.appearance.theme, crate::colors::DEFAULT_THEME).fonts;
        let SettingsDialog { window, draft, keymap, fonts, plugins: plugins_ui, discord_secret } = d;
        let actions = &self.actions;
        let plugins = &self.plugins;
        let catalog = crate::plugins::catalog();
        let mut plugin_actions = Vec::new();
        let mut appearance_action = None;
        // Тема, выбранная в черновике: её показывает инспектор и на её основе создаётся новая.
        let draft_theme = self.themes.resolve(&d_theme_id(&draft.appearance.theme), crate::colors::DEFAULT_THEME);
        let kit = theme_kit();
        let footer = window.show(ui, pal, max, &pages, &modified, dirty, &mut |ui, page, search| match page {
            appearance::PAGE => appearance_action = appearance::page(ui, pal, search, &mut draft.appearance, &cx).or(appearance_action),
            theme_tools::INSPECTOR_PAGE => theme_tools::inspector(ui, pal, search, &draft_theme, &kit),
            font_page::PAGE => fonts.ui(ui, pal, search, &mut draft.fonts, &draft_theme_fonts),
            keymap_page::PAGE => keymap.ui(ui, pal, actions, &mut draft.keymap, search),
            plugins_page::PAGE => plugins_ui.ui(ui, pal, search, plugins, &catalog, &mut draft.plugins, &mut plugin_actions),
            DISCORD_PAGE => discord_page(ui, pal, search, &mut draft.discord, discord_secret, project_open),
            _ => {}
        });
        for action in plugin_actions {
            self.cmds.push(plugin_cmd(action));
        }
        match appearance_action {
            Some(appearance::PageAction::CreateTheme) => {
                let now = Self::now(ui.ctx());
                match self.create_theme(&draft_theme, &kit) {
                    Ok((id, path)) => {
                        draft.appearance.theme = id;
                        crate::app::reveal_in_explorer(&path);
                        self.toast(t!("toast-theme-created", path = path.display().to_string()), false, now);
                    }
                    Err(e) => self.toast(t!("toast-theme-create-failed", error = e), true, now),
                }
            }
            Some(appearance::PageAction::OpenThemesFolder) => {
                if let Some(dir) = self.themes.dir() {
                    let _ = std::fs::create_dir_all(dir);
                    crate::app::open_folder(dir);
                }
            }
            None => {}
        }
        self.settings_page = d.window.page.clone();
        match footer {
            Some(Footer::Ok) => {
                self.apply_settings(d);
                true
            }
            Some(Footer::Apply) => {
                self.apply_settings(d);
                false
            }
            Some(Footer::Cancel) => true,
            Some(f @ (Footer::Export | Footer::Import)) => {
                // Сначала применяем черновик: экспорт должен взять то, что видно в окне.
                if self.draft_dirty(d) {
                    self.apply_settings(d);
                }
                self.cmds.push(if f == Footer::Export { Cmd::ExportSettings } else { Cmd::ImportSettings });
                true
            }
            None => false,
        }
    }

    /// Пункты «Найти действие»: общие действия и страницы настроек.
    fn find_items(&self) -> Vec<Item> {
        let mut items: Vec<Item> = self
            .actions
            .iter()
            .filter(|a| a.context != ActionContext::Editor && a.id != actions::FIND_ACTION)
            .map(|a| Item { id: a.id.clone(), title: keymap_page::title(a), group: forma_i18n::tr(&a.group), shortcut: self.keys(&a.id) })
            .collect();
        let settings = t!("find-action-settings");
        items.extend(self.settings_pages().into_iter().map(|p| Item { id: format!("{SETTINGS_ITEM}{}", p.id), title: p.title, group: settings.clone(), shortcut: String::new() }));
        items
    }

    pub(super) fn find_action_ui(&mut self, ui: &mut Ui, pal: &Palette, f: &mut FindAction) -> bool {
        ui.set_width(super::chrome::modal_width(ui.ctx(), 560.0));
        let items = self.find_items();
        let Some(id) = f.ui(ui, pal, &items) else { return false };
        match id.strip_prefix(SETTINGS_ITEM) {
            Some(page) => self.cmds.push(Cmd::OpenSettingsPage(page.to_owned())),
            None => self.cmds.push(Cmd::Action(id)),
        }
        true
    }

    pub(super) fn export_ui(&mut self, ui: &mut Ui, pal: &Palette, e: &mut ExportDialog, now: f64) -> bool {
        ui.set_width(super::chrome::modal_width(ui.ctx(), 460.0));
        title(ui, pal, &t!("settings-export-title"));
        ui.add(egui::Label::new(RichText::new(t!("settings-export-hint")).size(12.5).color(pal.text_muted)).wrap());
        ui.add_space(10.0);
        let infos = exportable();
        for (id, checked) in &mut e.checked {
            let Some((_, name, about)) = infos.iter().find(|(i, ..)| i == id) else { continue };
            ui.checkbox(checked, RichText::new(name).color(pal.text));
            ui.horizontal(|ui| {
                ui.add_space(26.0);
                ui.label(RichText::new(about).size(11.5).color(pal.text_faint));
            });
            ui.add_space(2.0);
        }
        ui.add_space(14.0);
        let mut close = false;
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let any = e.checked.iter().any(|(_, c)| *c);
            if ui.add_enabled_ui(any, |ui| sw::primary_button(ui, pal, &t!("settings-export-button"))).inner.clicked() {
                close = self.export_settings(e, now);
            }
            if ui.add(egui::Button::new(t!("settings-cancel")).min_size(vec2(84.0, 30.0))).clicked() {
                close = true;
            }
        });
        close
    }

    /// Спрашивает, куда сохранить архив, и пишет его. `true` — диалог можно закрыть.
    fn export_settings(&mut self, e: &ExportDialog, now: f64) -> bool {
        let mut archive = Archive::new(&APP);
        let on = |id: &str| e.checked.iter().any(|(i, c)| *i == id && *c);
        if on(Appearance::ID) {
            archive.add(&self.settings.appearance);
        }
        if on(Fonts::ID) {
            archive.add(&self.settings.fonts);
        }
        if on(Keymap::ID) {
            archive.add(&self.settings.keymap);
        }
        if on(Discord::ID) {
            archive.add(&self.settings.discord);
        }
        if on(PluginSettings::ID) {
            archive.add(&self.settings.plugins);
        }
        let name = export::default_file_name(&APP, archive.manifest.created_unix);
        let Some(path) = rfd::FileDialog::new().set_title(t!("dialog-export-settings")).set_file_name(name).add_filter("zip", &["zip"]).save_file() else {
            return false;
        };
        match archive.write(&path) {
            Ok(()) => self.toast(t!("toast-settings-exported", path = path.display().to_string()), false, now),
            Err(err) => self.toast(t!("toast-settings-export-failed", error = err.to_string()), true, now),
        }
        true
    }

    /// Спрашивает архив и показывает, что из него применится.
    pub(super) fn import_settings(&mut self, now: f64) {
        let Some(path) = rfd::FileDialog::new().set_title(t!("dialog-import-settings")).add_filter("zip", &["zip"]).pick_file() else { return };
        match read_import(&path, self) {
            Ok(dialog) => self.modal = Some(Modal::Import(Box::new(dialog))),
            Err(err) => self.toast(t!("toast-settings-import-failed", error = err), true, now),
        }
    }

    /// Что будет с компонентом архива. Раскладка клавиш сверяется со списком действий:
    /// сочетания для действий, которых здесь нет, пропускаются.
    fn import_status(&self, id: &str, value: &serde_json::Value) -> Status {
        let partial = |skipped: Vec<String>| if skipped.is_empty() { Status::Apply } else { Status::Partial(skipped) };
        match id {
            Appearance::ID => decode::<Appearance>(value).map_or_else(Status::Invalid, |(_, s)| partial(s)),
            Fonts::ID => decode::<Fonts>(value).map_or_else(Status::Invalid, |(_, s)| partial(s)),
            Discord::ID => decode::<Discord>(value).map_or_else(Status::Invalid, |(_, s)| partial(s)),
            PluginSettings::ID => decode::<PluginSettings>(value).map_or_else(Status::Invalid, |(_, s)| partial(s)),
            Keymap::ID => match decode::<Keymap>(value) {
                Ok((mut k, mut skipped)) => {
                    skipped.extend(k.retain_known(&self.actions));
                    partial(skipped)
                }
                Err(e) => Status::Invalid(e),
            },
            _ => Status::Unsupported,
        }
    }

    fn import_apply(&mut self, id: &str, value: &serde_json::Value) {
        match id {
            Appearance::ID => {
                if let Ok((a, _)) = decode::<Appearance>(value) {
                    let language_changed = a.effective_language() != self.settings.appearance.effective_language();
                    self.settings.appearance = a;
                    if language_changed {
                        forma_i18n::set_language(&self.settings.appearance.effective_language());
                    }
                    self.ctx.set_zoom_factor(self.settings.appearance.zoom);
                }
            }
            Discord::ID => {
                if let Ok((d, _)) = decode::<Discord>(value) {
                    self.settings.discord = d;
                }
            }
            PluginSettings::ID => {
                // Переносится, что выключено; недостающие плагины предлагаются к установке.
                if let Ok((p, _)) = decode::<PluginSettings>(value) {
                    self.settings.plugins.disabled = p.disabled;
                }
            }
            Fonts::ID => {
                if let Ok((f, _)) = decode::<Fonts>(value) {
                    self.settings.fonts = f;
                }
            }
            Keymap::ID => {
                if let Ok((mut k, _)) = decode::<Keymap>(value) {
                    k.retain_known(&self.actions);
                    self.settings.keymap = k;
                }
            }
            _ => {}
        }
    }

    pub(super) fn import_ui(&mut self, ui: &mut Ui, pal: &Palette, d: &mut ImportDialog) -> bool {
        ui.set_width(super::chrome::modal_width(ui.ctx(), 500.0));
        if let Some(report) = &d.report {
            title(ui, pal, &t!("settings-import-done-title"));
            for (id, status) in report {
                status_row(ui, pal, &component_title(id), status, None, true);
            }
            if !d.missing_plugins.is_empty() {
                ui.add_space(8.0);
                let names: Vec<String> = d.missing_plugins.iter().map(|id| crate::plugins::entry(id).map(|e| e.name).unwrap_or_else(|| id.clone())).collect();
                forma_ui::widgets::icon_label(ui, "plugin", t!("settings-import-plugins-missing", items = names.join(", ")), 12.5, pal.text_muted);
            }
            ui.add_space(14.0);
            let mut close = false;
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                close = sw::primary_button(ui, pal, &t!("action-done")).clicked();
                if !d.missing_plugins.is_empty() && forma_ui::icons::text_button(ui, "plugin", t!("settings-import-open-plugins")).clicked() {
                    self.cmds.push(Cmd::OpenSettingsPage(plugins_page::PAGE.into()));
                    close = true;
                }
            });
            return close;
        }
        title(ui, pal, &t!("settings-import-title"));
        let m = &d.archive.manifest;
        let date = export::date_text(m.created_unix);
        ui.label(RichText::new(t!("settings-import-from", app = m.app_title.clone(), version = m.app_version.clone(), date = date)).size(12.5).color(pal.text_muted));
        ui.add_space(10.0);
        for item in &mut d.items {
            let applicable = item.status.applicable();
            let check = applicable.then_some(&mut item.checked);
            status_row(ui, pal, &component_title(&item.id), &item.status, check, false);
        }
        ui.add_space(14.0);
        let mut close = false;
        let mut import = false;
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let any = d.items.iter().any(|i| i.checked && i.status.applicable());
            import = ui.add_enabled_ui(any, |ui| sw::primary_button(ui, pal, &t!("settings-import-button"))).inner.clicked();
            if ui.add(egui::Button::new(t!("settings-cancel")).min_size(vec2(84.0, 30.0))).clicked() {
                close = true;
            }
        });
        if import {
            let mut report = Vec::new();
            for item in &d.items {
                if item.checked && item.status.applicable() {
                    if let Some(v) = d.archive.components.get(&item.id) {
                        self.import_apply(&item.id, v);
                    }
                    report.push((item.id.clone(), item.status.clone()));
                } else if !item.status.applicable() {
                    report.push((item.id.clone(), item.status.clone()));
                }
            }
            self.settings.save(&mut self.store);
            d.report = Some(report);
        }
        close
    }
}

fn read_import(path: &Path, app: &MyceliumApp) -> Result<ImportDialog, String> {
    let archive = Archive::read(path).map_err(|e| e.to_string())?;
    let mut items: Vec<ImportItem> = archive
        .manifest
        .components
        .iter()
        .map(|id| {
            let status = match archive.components.get(id) {
                Some(v) => app.import_status(id, v),
                None => Status::Invalid(t!("settings-import-missing")),
            };
            ImportItem { id: id.clone(), checked: status.applicable(), status }
        })
        .collect();
    // Сначала то, что применится.
    items.sort_by_key(|i| !i.status.applicable());
    // Какие плагины стояли там, где делали архив, и не стоят здесь.
    let missing_plugins = archive
        .components
        .get(PluginSettings::ID)
        .and_then(|v| decode::<PluginSettings>(v).ok())
        .map(|(p, _)| p.installed.into_iter().filter(|id| app.plugins.get(id).is_none()).collect())
        .unwrap_or_default();
    Ok(ImportDialog { archive, items, missing_plugins, report: None })
}

/// Пустой id темы в настройках значит тему по умолчанию.
fn d_theme_id(id: &str) -> String {
    if id.is_empty() { crate::colors::DEFAULT_THEME.to_owned() } else { id.to_owned() }
}

/// Что тема может поменять в Forma Mycelium: ключи платформы и программы, классы подсветки.
fn theme_kit() -> theme_tools::Kit {
    theme_tools::Kit {
        groups: vec![(t!("settings-section-platform"), forma_ui::theme::Palette::TOKENS), (APP_TITLE.to_owned(), crate::colors::ModColors::TOKENS)],
        syntax: crate::highlight::CLASS_SELECTORS.iter().map(|(class, _)| *class).collect(),
    }
}

impl MyceliumApp {
    /// Создаёт свою тему из шаблона на основе `base`: «Моя тема», «Моя тема 2»…
    /// Возвращает id и путь к файлу.
    fn create_theme(&mut self, base: &forma_ui::theme::Theme, kit: &theme_tools::Kit) -> Result<(String, std::path::PathBuf), String> {
        let dir = self.themes.dir().ok_or("no themes folder")?.to_path_buf();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let base_name = t!("theme-new-name");
        let (name, id, path) = (1..)
            .map(|n| if n == 1 { base_name.clone() } else { format!("{base_name} {n}") })
            .map(|name| {
                let id = forma_ui::theme::slug(&name);
                let path = dir.join(format!("{id}.toml"));
                (name, id, path)
            })
            .find(|(_, id, path)| !path.exists() && self.themes.get(id).is_none())
            .ok_or("no free name")?;
        std::fs::write(&path, theme_tools::template(&name, &id, base, kit)).map_err(|e| format!("{}: {e}", path.display()))?;
        self.themes.scan();
        Ok((id, path))
    }
}

/// Команда для нажатого на странице «Плагины».
pub(super) fn plugin_cmd(action: plugins_page::Action) -> Cmd {
    match action {
        plugins_page::Action::Install(id) => Cmd::InstallPlugin(id),
        plugins_page::Action::InstallFile => Cmd::InstallPluginFile,
        plugins_page::Action::Uninstall(id) => Cmd::UninstallPlugin(id),
        plugins_page::Action::OpenFolder => Cmd::RevealPlugins,
    }
}

fn title(ui: &mut Ui, pal: &Palette, text: &str) {
    ui.label(RichText::new(text).size(18.0).family(forma_ui::fonts::semibold()).color(pal.heading));
    ui.add_space(8.0);
}

/// Строка компонента при импорте: галочка (если применимо), название и что с ним будет.
/// В отчёте после импорта (`done`) — что с ним стало.
fn status_row(ui: &mut Ui, pal: &Palette, name: &str, status: &Status, check: Option<&mut bool>, done: bool) {
    match check {
        Some(c) => {
            ui.checkbox(c, RichText::new(name).color(pal.text));
        }
        None => {
            ui.horizontal(|ui| {
                ui.add_space(26.0);
                ui.label(RichText::new(name).color(pal.text_muted));
            });
        }
    }
    let (icon, text, color) = match status {
        Status::Apply if done => ("success", t!("settings-import-applied"), pal.success),
        Status::Apply => ("success", t!("settings-import-apply"), pal.success),
        Status::Partial(skipped) if done => ("warning", t!("settings-import-applied-partial", items = skipped.join(", ")), pal.warning),
        Status::Partial(skipped) => ("warning", t!("settings-import-partial", items = skipped.join(", ")), pal.warning),
        Status::Unsupported => ("error", t!("settings-import-unsupported"), pal.text_faint),
        Status::Invalid(e) => ("error", t!("settings-import-invalid", error = e.clone()), pal.danger),
    };
    ui.horizontal(|ui| {
        ui.add_space(26.0);
        forma_ui::widgets::icon_label(ui, icon, text, 11.5, color);
    });
    ui.add_space(4.0);
}

/// Страница «Discord».
fn discord_page(ui: &mut Ui, pal: &Palette, search: &str, d: &mut Discord, secret: &mut bool, project_open: bool) {
    let def = Discord::default();
    let modified = d.enabled != def.enabled;
    if sw::check(ui, pal, search, &t!("settings-discord"), &mut d.enabled, modified) {
        d.enabled = def.enabled;
    }
    ui.add_enabled_ui(d.enabled && project_open, |ui| {
        let on = *secret;
        if sw::check(ui, pal, search, &t!("settings-discord-secret"), secret, on) {
            *secret = false;
        }
    });
    let hint = if project_open { t!("settings-discord-secret-hint") } else { t!("settings-discord-secret-no-project") };
    ui.horizontal(|ui| {
        ui.add_space(26.0);
        ui.add(egui::Label::new(RichText::new(hint).size(11.5).color(pal.text_faint)).wrap());
    });
}
