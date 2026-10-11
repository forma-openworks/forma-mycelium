//! Редактор «Проект»: дерево файлов проекта мода.
//! Пока это обычное дерево папок; на втором этапе оно разделится на «мой мод / Minecraft /
//! загрузчик / другие моды / библиотеки».

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use eframe::egui::{self, Align2, Color32, CornerRadius, CursorIcon, FontId, RichText, Sense, Ui, pos2, vec2};
use mycelium_i18n::t;

use crate::project::Project;
use crate::theme::Palette;

const ROW_H: f32 = 22.0;
const INDENT: f32 = 14.0;

#[derive(Clone)]
struct Entry {
    name: String,
    path: PathBuf,
    dir: bool,
}

/// Состояние дерева: прочитанные папки, раскрытые узлы, выделенный файл.
#[derive(Default)]
pub struct ProjectTree {
    root: Option<PathBuf>,
    cache: HashMap<PathBuf, Vec<Entry>>,
    expanded: HashSet<PathBuf>,
    selected: Option<PathBuf>,
}

impl ProjectTree {
    /// Папки читаются лениво — только когда их раскрывают.
    fn children(&mut self, dir: &Path) -> Vec<Entry> {
        self.cache.entry(dir.to_path_buf()).or_insert_with(|| read_dir_sorted(dir)).clone()
    }

    fn refresh(&mut self) {
        self.cache.clear();
    }

    pub fn ui(&mut self, ui: &mut Ui, pal: &Palette, project: &Project, tab_id: u64) {
        if self.root.as_deref() != Some(project.root.as_path()) {
            *self = Self { root: Some(project.root.clone()), ..Self::default() };
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.label(RichText::new(&project.name).family(crate::fonts::semibold()).color(pal.heading))
                .on_hover_text(project.root.display().to_string());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(6.0);
                if ui.button("⟳").on_hover_text(t!("project-refresh")).clicked() {
                    self.refresh();
                }
            });
        });
        ui.add_space(2.0);
        egui::ScrollArea::vertical().id_salt(("project-tree", tab_id)).auto_shrink(false).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let root = project.root.clone();
            self.rows(ui, pal, &root, 0);
            ui.add_space(8.0);
        });
    }

    fn rows(&mut self, ui: &mut Ui, pal: &Palette, dir: &Path, depth: usize) {
        for entry in self.children(dir) {
            let open = entry.dir && self.expanded.contains(&entry.path);
            let selected = self.selected.as_deref() == Some(entry.path.as_path());
            let resp = row(ui, pal, &entry, depth, open, selected);
            if resp.clicked() {
                if entry.dir {
                    if !self.expanded.remove(&entry.path) {
                        self.expanded.insert(entry.path.clone());
                    }
                } else {
                    self.selected = Some(entry.path.clone());
                }
            }
            if open {
                self.rows(ui, pal, &entry.path, depth + 1);
            }
        }
    }
}

/// Служебные папки сборки и IDE показываем приглушённо.
fn is_noise(name: &str) -> bool {
    name.starts_with('.') || matches!(name, "build" | "run" | "out" | "bin" | "gradle")
}

fn file_icon(name: &str, dir: bool, open: bool) -> (&'static str, Option<Color32>) {
    if dir {
        return (if open { "📂" } else { "📁" }, None);
    }
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "java" => ("☕", Some(Color32::from_rgb(232, 131, 58))),
        "kt" | "kts" => ("◆", Some(Color32::from_rgb(167, 139, 250))),
        "json" | "mcmeta" => ("{}", Some(Color32::from_rgb(250, 204, 21))),
        "png" => ("🖼", Some(Color32::from_rgb(52, 211, 153))),
        "toml" | "properties" | "gradle" => ("⚙", None),
        "obj" | "mtl" => ("🎲", Some(Color32::from_rgb(251, 146, 60))),
        _ => ("📄", None),
    }
}

fn row(ui: &mut Ui, pal: &Palette, entry: &Entry, depth: usize, open: bool, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.1);
    let painter = ui.painter();
    let bg_rect = rect.shrink2(vec2(4.0, 1.0));
    if selected {
        painter.rect_filled(bg_rect, CornerRadius::same(5), pal.accent_soft);
    } else if hover > 0.0 {
        painter.rect_filled(bg_rect, CornerRadius::same(5), pal.bg_hover.gamma_multiply(hover));
    }
    let mut x = rect.left() + 10.0 + depth as f32 * INDENT;
    let cy = rect.center().y;
    if entry.dir {
        painter.text(pos2(x + 5.0, cy), Align2::CENTER_CENTER, if open { "▾" } else { "▸" }, FontId::proportional(12.0), pal.text_faint);
    }
    x += 14.0;
    let (glyph, tint) = file_icon(&entry.name, entry.dir, open);
    let noise = is_noise(&entry.name);
    let base = if noise { pal.text_faint } else { pal.text };
    painter.text(pos2(x + 7.0, cy), Align2::CENTER_CENTER, glyph, FontId::proportional(12.5), tint.unwrap_or(pal.text_muted));
    x += 20.0;
    let color = if selected { pal.heading } else { base.lerp_to_gamma(pal.heading, hover * 0.5) };
    painter.with_clip_rect(rect).text(pos2(x, cy), Align2::LEFT_CENTER, &entry.name, FontId::proportional(13.0), color);
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

fn read_dir_sorted(dir: &Path) -> Vec<Entry> {
    let Ok(read) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut entries: Vec<Entry> = read
        .flatten()
        .map(|e| {
            let path = e.path();
            let dir = path.is_dir();
            Entry { name: e.file_name().to_string_lossy().into_owned(), path, dir }
        })
        .collect();
    // Папки сверху, внутри — по имени без учёта регистра.
    entries.sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_first_then_names() {
        let dir = std::env::temp_dir().join(format!("mycelium-tree-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::create_dir_all(dir.join("Assets")).unwrap();
        std::fs::write(dir.join("build.gradle"), "").unwrap();
        std::fs::write(dir.join("README.md"), "").unwrap();
        let names: Vec<String> = read_dir_sorted(&dir).into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["Assets", "src", "build.gradle", "README.md"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
