//! Редакторы — то, что живёт во вкладках областей. Каждый вид редактора можно открыть
//! в любой области и в любом количестве, как в Blender.

pub mod code;
mod code_assist;
pub mod console;
pub mod exceptions;
pub mod problem_ui;
mod project;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, Id, RichText, Ui};
use forma_actions::{Context as ActionContext, Keymap, Registry};
use forma_ui::fonts::Fonts;
use forma_i18n::t;
use forma_layout::TabViewer;
use mycelium_project::Project;
use serde::{Deserialize, Serialize};

pub use code::Documents;
pub use project::ProjectTree;

use crate::app::Cmd;
use crate::run::{Console, RunConfig};
use crate::lsp::JavaLsp;
use crate::sync::SyncState;
use forma_ui::theme::Palette;
use forma_ui::widgets;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EditorKind {
    Project,
    Code,
    ModMap,
    Exceptions,
    Console,
    ModelPreview,
}

impl EditorKind {
    pub const ALL: [EditorKind; 6] =
        [EditorKind::Project, EditorKind::Code, EditorKind::ModMap, EditorKind::Exceptions, EditorKind::Console, EditorKind::ModelPreview];

    pub fn title(self) -> String {
        match self {
            EditorKind::Project => t!("editor-project"),
            EditorKind::Code => t!("editor-code"),
            EditorKind::ModMap => t!("editor-mod-map"),
            EditorKind::Exceptions => t!("editor-exceptions"),
            EditorKind::Console => t!("editor-console"),
            EditorKind::ModelPreview => t!("editor-model-preview"),
        }
    }

    /// Что будет уметь редактор — показывается, пока он ещё заглушка.
    fn about(self) -> String {
        match self {
            EditorKind::Project => t!("editor-project-about"),
            EditorKind::Code => t!("editor-code-about"),
            EditorKind::ModMap => t!("editor-mod-map-about"),
            EditorKind::Exceptions => t!("editor-exceptions-about"),
            EditorKind::Console => t!("editor-console-about"),
            EditorKind::ModelPreview => t!("editor-model-preview-about"),
        }
    }

    /// На каком этапе плана редактор наполнится содержимым.
    fn stage(self) -> Option<u32> {
        match self {
            EditorKind::Project | EditorKind::Code | EditorKind::Exceptions | EditorKind::Console => None,
            EditorKind::ModMap => Some(3),
            EditorKind::ModelPreview => Some(4),
        }
    }

    /// Имя значка (см. `forma_ui::icons`).
    pub fn icon(self) -> &'static str {
        match self {
            EditorKind::Project => "editor-project",
            EditorKind::Code => "editor-code",
            EditorKind::ModMap => "editor-map",
            EditorKind::Exceptions => "editor-exceptions",
            EditorKind::Console => "editor-console",
            EditorKind::ModelPreview => "editor-model",
        }
    }

    pub fn color(self, _pal: &Palette) -> Color32 {
        let c = crate::colors::current();
        match self {
            EditorKind::Project => c.kind_project,
            EditorKind::Code => c.kind_code,
            EditorKind::ModMap => c.kind_map,
            EditorKind::Exceptions => c.kind_exceptions,
            EditorKind::Console => c.kind_console,
            EditorKind::ModelPreview => c.kind_model,
        }
    }
}

/// Вкладка-редактор. Номер нужен, чтобы два редактора одного вида не делили состояние.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EditorTab {
    pub id: u64,
    pub kind: EditorKind,
    /// Файл вкладки «Код» — путь относительно корня проекта (или путь к jar, если задан `entry`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
    /// Файл внутри jar (`net/minecraft/world/item/Item.java`): вкладка только для чтения.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
}

impl EditorTab {
    pub fn new(kind: EditorKind, next_id: &mut u64) -> Self {
        *next_id += 1;
        Self { id: *next_id, kind, file: None, entry: None }
    }

    pub fn file(path: PathBuf, next_id: &mut u64) -> Self {
        let mut tab = Self::new(EditorKind::Code, next_id);
        tab.file = Some(path);
        tab
    }

    pub fn jar(jar: PathBuf, entry: String, next_id: &mut u64) -> Self {
        let mut tab = Self::file(jar, next_id);
        tab.entry = Some(entry);
        tab
    }

    /// Ключ документа вкладки в `Documents`.
    pub fn doc_key(&self, root: &Path) -> Option<PathBuf> {
        let file = self.file.as_ref()?;
        Some(match &self.entry {
            Some(entry) => code::jar_key(file, entry),
            None => root.join(file),
        })
    }

    /// Та же ли это вкладка файла.
    pub fn shows(&self, file: &Path, entry: Option<&str>) -> bool {
        self.file.as_deref() == Some(file) && self.entry.as_deref() == entry
    }
}

/// Состояние вкладок, которое живёт только пока открыта программа (выбор, фильтры).
#[derive(Default)]
pub struct EditorStates {
    console: HashMap<u64, console::ConsoleView>,
    exceptions: HashMap<u64, exceptions::ExceptionsView>,
}

/// Раскладка клавиш для редакторов, которые ловят свои сочетания сами.
#[derive(Clone, Copy)]
pub struct Keys<'a> {
    pub actions: &'a Registry,
    pub keymap: &'a Keymap,
}

/// Всё, что нужно редакторам для отрисовки. Собирается заново на каждом кадре.
pub struct Editors<'a> {
    pub pal: &'a Palette,
    pub project: Option<&'a Project>,
    pub tree: &'a mut ProjectTree,
    pub recent: &'a [PathBuf],
    pub actions: &'a Registry,
    pub keymap: &'a Keymap,
    /// Работают ли общие сочетания (пока открыт диалог — нет).
    pub shortcuts: bool,
    pub next_tab_id: &'a mut u64,
    pub cmds: &'a mut Vec<Cmd>,
    pub docs: &'a mut Documents,
    pub console: &'a Console,
    pub sync: &'a SyncState,
    pub lsp: Option<&'a mut JavaLsp>,
    pub configs: &'a [RunConfig],
    pub run_config: &'a mut Option<String>,
    pub states: &'a mut EditorStates,
    pub fonts: &'a Fonts,
    pub now: f64,
}

impl TabViewer for Editors<'_> {
    type Tab = EditorTab;

    fn title(&mut self, tab: &EditorTab) -> String {
        if let Some(entry) = &tab.entry {
            return entry.rsplit('/').next().unwrap_or(entry).to_owned();
        }
        match &tab.file {
            Some(file) => {
                let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let dirty = self.project.is_some_and(|p| self.docs.is_dirty(&p.root.join(file)));
                if dirty { format!("{name} ●") } else { name }
            }
            None => tab.kind.title(),
        }
    }

    fn icon(&mut self, tab: &EditorTab) -> Option<(String, Color32)> {
        if let Some(entry) = &tab.entry {
            // Исходники и ресурсы библиотек — приглушённым значком: это чужой код.
            let (glyph, _) = project::file_icon(entry, false, false);
            return Some((glyph.to_owned(), self.pal.text_faint));
        }
        if let Some(file) = &tab.file {
            let (glyph, tint) = project::file_icon(&file.to_string_lossy(), false, false);
            return Some((glyph.to_owned(), tint.unwrap_or(self.pal.text_muted)));
        }
        Some((tab.kind.icon().to_owned(), tab.kind.color(self.pal)))
    }

    fn id(&mut self, tab: &EditorTab) -> Id {
        Id::new(("mycelium-editor", tab.id))
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut EditorTab) {
        let pal = self.pal;
        match tab.kind {
            EditorKind::Project => match self.project {
                Some(project) => self.tree.ui(ui, pal, project, self.sync, tab.id, self.cmds),
                None => no_project(ui, pal, self.recent, self.cmds),
            },
            EditorKind::Code => match (self.project, &tab.file, &tab.entry) {
                (Some(_), Some(jar), Some(entry)) => {
                    // Путь в заголовке: jar › путь внутри.
                    let jar_name = jar.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    let rel = format!("{jar_name}/{entry}");
                    let doc = self.docs.open_jar(jar, entry);
                    let keys = Keys { actions: self.actions, keymap: self.keymap };
                    code::ui(ui, doc, &rel, pal, tab.id, self.fonts, self.now, None, keys, self.cmds);
                }
                (Some(project), Some(file), None) => {
                    let rel = file.to_string_lossy().into_owned();
                    let doc = self.docs.open(&project.root.join(file));
                    let keys = Keys { actions: self.actions, keymap: self.keymap };
                    code::ui(ui, doc, &rel, pal, tab.id, self.fonts, self.now, self.lsp.as_deref_mut(), keys, self.cmds);
                }
                _ => empty_state(ui, pal, EditorKind::Code.icon(), &t!("code-empty-title"), &t!("code-empty-hint")),
            },
            EditorKind::Console => {
                let view = self.states.console.entry(tab.id).or_default();
                let libs = self.project.and_then(|p| p.libraries.as_deref());
                console::ui(ui, pal, self.fonts, view, self.console, libs, self.configs, self.run_config, self.project.is_some(), self.now, self.cmds);
            }
            EditorKind::Exceptions => {
                let view = self.states.exceptions.entry(tab.id).or_default();
                let libs = self.project.and_then(|p| p.libraries.as_deref());
                exceptions::ui(ui, pal, view, self.console, libs, self.cmds);
            }
            kind => placeholder(ui, pal, kind),
        }
    }

    fn add_menu(&mut self, ui: &mut Ui) -> Option<EditorTab> {
        let mut picked = None;
        for kind in EditorKind::ALL {
            if forma_ui::icons::item(ui, kind.icon(), kind.title(), false).clicked() {
                picked = Some(EditorTab::new(kind, self.next_tab_id));
            }
        }
        picked
    }

    fn duplicate(&mut self, tab: &EditorTab) -> EditorTab {
        let mut copy = EditorTab::new(tab.kind, self.next_tab_id);
        copy.file = tab.file.clone();
        copy.entry = tab.entry.clone();
        copy
    }

    fn on_close(&mut self, tab: EditorTab) {
        // Закрытая вкладка файла сохраняет его сразу, не дожидаясь автосохранения.
        if let Some(key) = self.project.and_then(|p| tab.doc_key(&p.root)) {
            if let Some(doc) = self.docs.get_mut(&key) {
                if let Err(e) = doc.save() {
                    log::warn!("{e}");
                }
            }
        }
        self.states.console.remove(&tab.id);
        self.states.exceptions.remove(&tab.id);
    }

    fn window_input(&mut self, ui: &mut Ui) {
        if !self.shortcuts {
            return;
        }
        let fired = ui.input_mut(|i| forma_actions::triggered(i, self.actions, self.keymap, ActionContext::Global));
        self.cmds.extend(fired.iter().filter_map(|id| crate::app::action_cmd(id)));
    }
}

/// Содержимое по центру, которое в низкой области прокручивается, а не обрезается.
fn centered_scroll(ui: &mut Ui, add: impl FnOnce(&mut Ui, f32)) {
    let height = ui.available_height();
    egui::ScrollArea::vertical().id_salt("mycelium-centered").auto_shrink(false).show(ui, |ui| {
        ui.vertical_centered(|ui| add(ui, height));
    });
}

/// Крупный значок по центру строки (в пустых состояниях и заглушках).
pub fn big_icon(ui: &mut Ui, name: &str, size: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    forma_ui::icons::paint(ui.painter(), rect, name, color);
}

/// Пустое состояние редактора: значок, заголовок, подсказка.
pub fn empty_state(ui: &mut Ui, pal: &Palette, icon: &str, title: &str, hint: &str) {
    centered_scroll(ui, |ui, height| {
        ui.add_space((height * 0.3 - 50.0).max(12.0));
        big_icon(ui, icon, 30.0, pal.text_faint);
        ui.add_space(4.0);
        ui.label(RichText::new(title).size(15.0).family(forma_ui::fonts::semibold()).color(pal.heading));
        ui.add_space(2.0);
        widgets::centered_text(ui, hint, 13.0, pal.text_muted, 380.0);
        ui.add_space(12.0);
    });
}

/// Заглушка: что будет в редакторе и на каком этапе он появится.
fn placeholder(ui: &mut Ui, pal: &Palette, kind: EditorKind) {
    centered_scroll(ui, |ui, height| {
        ui.add_space((height * 0.3 - 70.0).max(12.0));
        big_icon(ui, kind.icon(), 38.0, kind.color(pal));
        ui.add_space(6.0);
        ui.label(RichText::new(kind.title()).size(18.0).family(forma_ui::fonts::semibold()).color(pal.heading));
        ui.add_space(4.0);
        widgets::centered_text(ui, &kind.about(), 13.5, pal.text_muted, 380.0);
        if let Some(stage) = kind.stage() {
            ui.add_space(10.0);
            badge(ui, pal, &t!("editor-coming-stage", stage = stage));
        }
        ui.add_space(12.0);
    });
}

/// Небольшая плашка с текстом в акцентном цвете — по ширине текста, а не всей области.
fn badge(ui: &mut Ui, pal: &Palette, text: &str) {
    let galley = widgets::elide(ui, text, egui::FontId::proportional(12.0), pal.accent, ui.available_width() - 24.0);
    let (rect, _) = ui.allocate_exact_size(galley.size() + egui::vec2(20.0, 8.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, egui::CornerRadius::same(10), pal.accent_soft);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, pal.accent);
}

/// Редактор «Проект», пока проект не открыт.
fn no_project(ui: &mut Ui, pal: &Palette, recent: &[PathBuf], cmds: &mut Vec<Cmd>) {
    centered_scroll(ui, |ui, height| {
        ui.add_space((height * 0.25 - 60.0).max(12.0));
        big_icon(ui, EditorKind::Project.icon(), 34.0, pal.accent);
        ui.add_space(4.0);
        ui.label(RichText::new(t!("project-none-title")).size(16.0).family(forma_ui::fonts::semibold()).color(pal.heading));
        ui.add_space(4.0);
        widgets::centered_text(ui, &t!("project-none-hint"), 13.0, pal.text_muted, 320.0);
        ui.add_space(10.0);
        let width = (ui.available_width() - 24.0).clamp(80.0, 200.0);
        if ui.add(egui::Button::new(t!("action-open-project")).fill(pal.accent_soft).min_size(egui::vec2(width, 30.0)).truncate()).clicked() {
            cmds.push(Cmd::OpenProjectDialog);
        }
        let recent: Vec<&PathBuf> = recent.iter().filter(|p| p.is_dir()).take(5).collect();
        if !recent.is_empty() {
            ui.add_space(14.0);
            ui.label(RichText::new(t!("welcome-recent")).size(12.0).color(pal.text_faint));
            for path in recent {
                let name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                if ui.add(egui::Button::new(RichText::new(name).color(pal.text)).frame(false).truncate()).on_hover_text(path.display().to_string()).clicked() {
                    cmds.push(Cmd::OpenProject(path.clone()));
                }
            }
        }
        ui.add_space(12.0);
    });
}
