//! Редакторы — то, что живёт во вкладках областей. Каждый вид редактора можно открыть
//! в любой области и в любом количестве, как в Blender.

mod project;

use std::path::PathBuf;

use eframe::egui::{self, Color32, Id, RichText, Ui};
use mycelium_i18n::t;
use mycelium_layout::TabViewer;
use serde::{Deserialize, Serialize};

pub use project::ProjectTree;

use crate::app::Cmd;
use crate::project::Project;
use crate::theme::Palette;
use crate::widgets;

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
            EditorKind::Project => None,
            EditorKind::Code | EditorKind::Exceptions | EditorKind::Console => Some(2),
            EditorKind::ModMap => Some(3),
            EditorKind::ModelPreview => Some(4),
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            EditorKind::Project => "📁",
            EditorKind::Code => "📝",
            EditorKind::ModMap => "🕸",
            EditorKind::Exceptions => "⚠",
            EditorKind::Console => "🖥",
            EditorKind::ModelPreview => "🎲",
        }
    }

    pub fn color(self, pal: &Palette) -> Color32 {
        match self {
            EditorKind::Project => pal.accent,
            EditorKind::Code => Color32::from_rgb(96, 165, 250),
            EditorKind::ModMap => Color32::from_rgb(192, 132, 252),
            EditorKind::Exceptions => pal.warning,
            EditorKind::Console => Color32::from_rgb(45, 212, 191),
            EditorKind::ModelPreview => Color32::from_rgb(251, 146, 60),
        }
    }
}

/// Вкладка-редактор. Номер нужен, чтобы два редактора одного вида не делили состояние.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EditorTab {
    pub id: u64,
    pub kind: EditorKind,
}

impl EditorTab {
    pub fn new(kind: EditorKind, next_id: &mut u64) -> Self {
        *next_id += 1;
        Self { id: *next_id, kind }
    }
}

/// Всё, что нужно редакторам для отрисовки. Собирается заново на каждом кадре.
pub struct Editors<'a> {
    pub pal: &'a Palette,
    pub project: Option<&'a Project>,
    pub tree: &'a mut ProjectTree,
    pub recent: &'a [PathBuf],
    pub next_tab_id: &'a mut u64,
    pub cmds: &'a mut Vec<Cmd>,
}

impl TabViewer for Editors<'_> {
    type Tab = EditorTab;

    fn title(&mut self, tab: &EditorTab) -> String {
        tab.kind.title()
    }

    fn icon(&mut self, tab: &EditorTab) -> Option<(String, Color32)> {
        Some((tab.kind.icon().to_owned(), tab.kind.color(self.pal)))
    }

    fn id(&mut self, tab: &EditorTab) -> Id {
        Id::new(("mycelium-editor", tab.id))
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut EditorTab) {
        match tab.kind {
            EditorKind::Project => match self.project {
                Some(project) => self.tree.ui(ui, self.pal, project, tab.id),
                None => no_project(ui, self.pal, self.recent, self.cmds),
            },
            kind => placeholder(ui, self.pal, kind),
        }
    }

    fn add_menu(&mut self, ui: &mut Ui) -> Option<EditorTab> {
        let mut picked = None;
        for kind in EditorKind::ALL {
            let text = RichText::new(format!("{}  {}", kind.icon(), kind.title()));
            if ui.button(text).clicked() {
                picked = Some(EditorTab::new(kind, self.next_tab_id));
            }
        }
        picked
    }

    fn duplicate(&mut self, tab: &EditorTab) -> EditorTab {
        EditorTab::new(tab.kind, self.next_tab_id)
    }
}

/// Заглушка: что будет в редакторе и на каком этапе он появится.
fn placeholder(ui: &mut Ui, pal: &Palette, kind: EditorKind) {
    let height = ui.available_height();
    ui.vertical_centered(|ui| {
        ui.add_space((height * 0.3 - 70.0).max(12.0));
        ui.label(RichText::new(kind.icon()).size(38.0).color(kind.color(pal)));
        ui.add_space(6.0);
        ui.label(RichText::new(kind.title()).size(18.0).family(crate::fonts::semibold()).color(pal.heading));
        ui.add_space(4.0);
        widgets::centered_text(ui, &kind.about(), 13.5, pal.text_muted, 380.0);
        if let Some(stage) = kind.stage() {
            ui.add_space(10.0);
            badge(ui, pal, &t!("editor-coming-stage", stage = stage));
        }
    });
}

/// Небольшая плашка с текстом в акцентном цвете — по ширине текста, а не всей области.
fn badge(ui: &mut Ui, pal: &Palette, text: &str) {
    let galley = ui.painter().layout_no_wrap(text.to_owned(), egui::FontId::proportional(12.0), pal.accent);
    let (rect, _) = ui.allocate_exact_size(galley.size() + egui::vec2(20.0, 8.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, egui::CornerRadius::same(10), pal.accent_soft);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, pal.accent);
}

/// Редактор «Проект», пока проект не открыт.
fn no_project(ui: &mut Ui, pal: &Palette, recent: &[PathBuf], cmds: &mut Vec<Cmd>) {
    let height = ui.available_height();
    ui.vertical_centered(|ui| {
        ui.add_space((height * 0.25 - 60.0).max(12.0));
        ui.label(RichText::new(EditorKind::Project.icon()).size(34.0).color(pal.accent));
        ui.add_space(4.0);
        ui.label(RichText::new(t!("project-none-title")).size(16.0).family(crate::fonts::semibold()).color(pal.heading));
        ui.add_space(4.0);
        widgets::centered_text(ui, &t!("project-none-hint"), 13.0, pal.text_muted, 320.0);
        ui.add_space(10.0);
        if ui.add(egui::Button::new(t!("action-open-project")).fill(pal.accent_soft).min_size(egui::vec2(200.0, 30.0))).clicked() {
            cmds.push(Cmd::OpenProjectDialog);
        }
        let recent: Vec<&PathBuf> = recent.iter().filter(|p| p.is_dir()).take(5).collect();
        if !recent.is_empty() {
            ui.add_space(14.0);
            ui.label(RichText::new(t!("welcome-recent")).size(12.0).color(pal.text_faint));
            for path in recent {
                let name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                if ui.add(egui::Button::new(RichText::new(name).color(pal.text)).frame(false)).on_hover_text(path.display().to_string()).clicked() {
                    cmds.push(Cmd::OpenProject(path.clone()));
                }
            }
        }
    });
}
