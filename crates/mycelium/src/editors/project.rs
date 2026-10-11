//! Редактор «Проект»: сведения о моде и дерево файлов. Скрытые файлы и папки
//! (с точки или с атрибутом «скрытый», включая `.mycelium`) показываются по кнопке.
//! Раскрытые папки и эта кнопка запоминаются в проекте.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui::{self, Align2, Color32, CornerRadius, CursorIcon, FontId, RichText, Sense, Ui, pos2, vec2};
use mycelium_i18n::t;
use mycelium_project::libraries::{GroupKind, Node, Part};
use mycelium_project::{Libraries, Origin, Project, store};

use super::problem_ui::{chip, origin_color};
use crate::app::Cmd;
use crate::sync::SyncState;
use crate::theme::Palette;

const ROW_H: f32 = 22.0;
const INDENT: f32 = 14.0;

#[derive(Clone)]
struct Entry {
    name: String,
    path: PathBuf,
    dir: bool,
    hidden: bool,
}

/// Состояние дерева: прочитанные папки, раскрытые узлы, выделенный файл.
#[derive(Default)]
pub struct ProjectTree {
    root: Option<PathBuf>,
    cache: HashMap<PathBuf, Vec<Entry>>,
    expanded: HashSet<PathBuf>,
    selected: Option<PathBuf>,
    pub show_hidden: bool,
    /// Раздел «Внешние библиотеки» раскрыт.
    pub libs_open: bool,
    /// Раскрытые узлы библиотек: `g0`, `g0/net/minecraft/`, `g0#m`, `j12`, `j12/com/google/`.
    lib_open: HashSet<String>,
    lib_selected: Option<String>,
    /// Посчитанные дочерние узлы библиотек и для какого индекса библиотек они посчитаны.
    lib_cache: HashMap<String, Arc<Vec<Node>>>,
    lib_cache_of: usize,
}

impl ProjectTree {
    /// Дерево проекта с сохранённым состоянием: раскрытые папки (относительно корня) и скрытые файлы.
    pub fn restore(root: &Path, expanded: &[String], show_hidden: bool) -> Self {
        Self {
            root: Some(root.to_path_buf()),
            expanded: expanded.iter().map(|rel| root.join(rel)).collect(),
            show_hidden,
            ..Self::default()
        }
    }

    /// Раскрытые папки относительно корня, через «/» — для сохранения в проекте.
    pub fn expanded_relative(&self) -> Vec<String> {
        let Some(root) = &self.root else { return Vec::new() };
        let mut out: Vec<String> = self
            .expanded
            .iter()
            .filter_map(|p| p.strip_prefix(root).ok())
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .collect();
        out.sort();
        out
    }

    /// Папки читаются лениво — только когда их раскрывают.
    fn children(&mut self, dir: &Path) -> Vec<Entry> {
        self.cache.entry(dir.to_path_buf()).or_insert_with(|| read_dir_sorted(dir)).clone()
    }

    pub fn refresh(&mut self) {
        self.cache.clear();
    }

    /// Раскрывает папки до файла и выделяет его.
    pub fn reveal(&mut self, path: &Path) {
        let mut cur = path.parent();
        while let Some(dir) = cur {
            if self.root.as_deref().is_some_and(|r| !dir.starts_with(r)) || Some(dir) == self.root.as_deref() {
                break;
            }
            self.expanded.insert(dir.to_path_buf());
            cur = dir.parent();
        }
        self.selected = Some(path.to_path_buf());
    }

    pub fn ui(&mut self, ui: &mut Ui, pal: &Palette, project: &Project, sync: &SyncState, tab_id: u64, cmds: &mut Vec<Cmd>) {
        if self.root.as_deref() != Some(project.root.as_path()) {
            *self = Self { root: Some(project.root.clone()), show_hidden: self.show_hidden, libs_open: self.libs_open, ..Self::default() };
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.label(RichText::new(project.name()).family(crate::fonts::semibold()).color(pal.heading))
                .on_hover_text(project.root.display().to_string());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(6.0);
                if ui.button("⟳").on_hover_text(t!("project-refresh")).clicked() {
                    self.refresh();
                    cmds.push(Cmd::Redetect);
                    cmds.push(Cmd::SyncGradle);
                }
                if ui.button("⊟").on_hover_text(t!("project-collapse-all")).clicked() {
                    self.expanded.clear();
                }
                let hidden = ui.add(egui::Button::new("👁").selected(self.show_hidden)).on_hover_text(if self.show_hidden {
                    t!("project-hide-hidden")
                } else {
                    t!("project-show-hidden")
                });
                if hidden.clicked() {
                    self.show_hidden = !self.show_hidden;
                }
            });
        });
        mod_info(ui, pal, project);
        ui.add_space(4.0);
        egui::ScrollArea::vertical().id_salt(("project-tree", tab_id)).auto_shrink(false).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let root = project.root.clone();
            self.rows(ui, pal, &root, 0, cmds);
            if project.info.gradle {
                ui.add_space(4.0);
                self.library_rows(ui, pal, project, sync, cmds);
            }
            ui.add_space(8.0);
        });
    }

    /// «Внешние библиотеки»: Minecraft, загрузчик, другие моды, библиотеки и JDK.
    fn library_rows(&mut self, ui: &mut Ui, pal: &Palette, project: &Project, sync: &SyncState, cmds: &mut Vec<Cmd>) {
        let libs = project.libraries.as_deref();
        // Индекс пересобрали — посчитанные узлы больше не годятся.
        let ptr = libs.map_or(0, |l| l as *const Libraries as usize);
        if ptr != self.lib_cache_of {
            self.lib_cache.clear();
            self.lib_cache_of = ptr;
        }

        let (status, status_color) = match sync {
            SyncState::Running { .. } => (Some(format!("⟳ {}", t!("libs-syncing"))), pal.accent),
            SyncState::Indexing => (Some(format!("⟳ {}", t!("libs-indexing"))), pal.accent),
            SyncState::Failed { .. } => (Some(format!("⚠ {}", t!("libs-sync-failed"))), pal.danger),
            SyncState::Idle => (libs.map(|l| l.libs.len().to_string()), pal.text_faint),
        };
        let header = tree_row(ui, pal, 0, Some(self.libs_open), "📚", pal.text_muted, &t!("libs-title"), pal.text, status.as_deref().map(|s| (s, status_color)), false);
        let header = match sync {
            SyncState::Failed { message } => header.on_hover_text(format!("{message}\n\n{}", t!("libs-sync-failed-hint"))),
            _ => header,
        };
        if header.clicked() {
            self.libs_open = !self.libs_open;
        }
        crate::theme::context_menu(&header, |ui| {
            if ui.button(t!("libs-sync")).clicked() {
                cmds.push(Cmd::SyncGradle);
            }
        });
        if !self.libs_open {
            return;
        }
        let Some(libs) = libs else {
            if !matches!(sync, SyncState::Running { .. } | SyncState::Indexing) {
                ui.horizontal(|ui| {
                    ui.add_space(10.0 + INDENT + 14.0);
                    if ui.link(t!("libs-sync")).clicked() {
                        cmds.push(Cmd::SyncGradle);
                    }
                });
            }
            return;
        };

        for (gi, group) in libs.groups.iter().enumerate() {
            let key = format!("g{gi}");
            let open = self.lib_open.contains(&key);
            let (title, origin) = match &group.kind {
                GroupKind::Minecraft => (format!("Minecraft {}", group.version.as_deref().unwrap_or("")), Origin::Minecraft),
                GroupKind::Loader(l) => (format!("{} {}", l.name(), group.version.as_deref().unwrap_or("")), mycelium_project::origin::loader_origin(*l)),
                GroupKind::Mods => (t!("libs-mods"), Origin::OtherMod(None)),
                GroupKind::Libraries => (t!("libs-libraries"), Origin::Library),
            };
            let count = libs.group_size(group).to_string();
            let resp = tree_row(ui, pal, 1, Some(open), "●", origin_color(pal, &origin), title.trim(), pal.text, Some((&count, pal.text_faint)), false);
            if resp.clicked() {
                toggle(&mut self.lib_open, &key);
            }
            if !open {
                continue;
            }
            if !group.parts.is_empty() {
                self.content_rows(ui, pal, libs, &key, &group.parts, "", 2, cmds);
            }
            // Отдельные jar: у Minecraft и загрузчика — свёрнутой подгруппой, у модов и библиотек — сразу списком.
            match group.kind {
                GroupKind::Minecraft | GroupKind::Loader(_) if !group.members.is_empty() => {
                    let sub = format!("{key}#m");
                    let sub_open = self.lib_open.contains(&sub);
                    let label = if group.kind == GroupKind::Minecraft { t!("libs-mojang") } else { t!("libs-loader-parts") };
                    let n = group.members.len().to_string();
                    if tree_row(ui, pal, 2, Some(sub_open), "📦", pal.text_muted, &label, pal.text_muted, Some((&n, pal.text_faint)), false).clicked() {
                        toggle(&mut self.lib_open, &sub);
                    }
                    if sub_open {
                        for &m in &group.members {
                            self.jar_rows(ui, pal, libs, m, 3, cmds);
                        }
                    }
                }
                _ => {
                    for &m in &group.members {
                        self.jar_rows(ui, pal, libs, m, 2, cmds);
                    }
                }
            }
        }
        if let Some(jdk) = &libs.jdk {
            let resp = tree_row(ui, pal, 1, None, "☕", origin_color(pal, &Origin::Java), &format!("JDK {}", jdk.version), pal.text, None, false);
            let resp = resp.on_hover_text(jdk.home.display().to_string());
            crate::theme::context_menu(&resp, |ui| {
                if ui.button(t!("project-reveal")).clicked() {
                    cmds.push(Cmd::Reveal(jdk.home.clone()));
                }
            });
        }
    }

    /// Один jar: строка с именем и версией, внутри — его содержимое.
    fn jar_rows(&mut self, ui: &mut Ui, pal: &Palette, libs: &Libraries, lib: usize, depth: usize, cmds: &mut Vec<Cmd>) {
        let Some(l) = libs.libs.get(lib) else { return };
        let key = format!("j{lib}");
        let open = self.lib_open.contains(&key);
        let version = l.version.clone().unwrap_or_default();
        let resp = tree_row(ui, pal, depth, Some(open), "🗄", pal.text_faint, &l.name, pal.text, Some((&version, pal.text_faint)), false);
        let coords = match (&l.group, &l.version) {
            (Some(g), Some(v)) => format!("{g}:{}:{v}", l.jar.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()),
            _ => l.jar.display().to_string(),
        };
        let sources = if l.sources.is_some() { t!("libs-has-sources") } else { t!("libs-no-sources") };
        let resp = resp.on_hover_text(format!("{coords}\n{sources}"));
        if resp.clicked() {
            toggle(&mut self.lib_open, &key);
        }
        crate::theme::context_menu(&resp, |ui| {
            if ui.button(t!("project-reveal")).clicked() {
                cmds.push(Cmd::Reveal(l.jar.clone()));
            }
            if ui.button(t!("project-copy-path")).clicked() {
                ui.ctx().copy_text(l.jar.display().to_string());
            }
        });
        if open {
            let parts = [Part { lib, prefixes: Vec::new() }];
            self.content_rows(ui, pal, libs, &key, &parts, "", depth + 1, cmds);
        }
    }

    /// Содержимое части дерева: пакеты, папки ресурсов и файлы.
    #[allow(clippy::too_many_arguments)]
    fn content_rows(&mut self, ui: &mut Ui, pal: &Palette, libs: &Libraries, base: &str, parts: &[Part], dir: &str, depth: usize, cmds: &mut Vec<Cmd>) {
        let cache_key = format!("{base}/{dir}");
        let nodes = self.lib_cache.entry(cache_key).or_insert_with(|| Arc::new(libs.children(parts, dir))).clone();
        for node in nodes.iter() {
            let key = format!("{base}/{}", node.path);
            let selected = self.lib_selected.as_deref() == Some(key.as_str());
            if node.dir {
                let open = self.lib_open.contains(&key);
                let resource = node.path.starts_with("assets/") || node.path.starts_with("data/") || node.path.starts_with("META-INF/");
                let glyph = if resource { if open { "📂" } else { "📁" } } else { "▣" };
                if tree_row(ui, pal, depth, Some(open), glyph, pal.text_faint, &node.name, pal.text, None, selected).clicked() {
                    toggle(&mut self.lib_open, &key);
                }
                if open {
                    self.content_rows(ui, pal, libs, base, parts, &node.path, depth + 1, cmds);
                }
                continue;
            }
            let (glyph, tint) = if node.is_class() { ("☕", Some(pal.text_muted)) } else { file_icon(&node.name, false, false) };
            let resp = tree_row(ui, pal, depth, None, glyph, tint.unwrap_or(pal.text_faint), &node.name, pal.text, None, selected);
            if resp.clicked() {
                self.lib_selected = Some(key.clone());
            }
            if resp.double_clicked() {
                cmds.push(Cmd::OpenLibraryEntry { lib: node.lib, entry: node.path.clone() });
            }
            crate::theme::context_menu(&resp, |ui| {
                if ui.button(t!("project-open")).clicked() {
                    cmds.push(Cmd::OpenLibraryEntry { lib: node.lib, entry: node.path.clone() });
                }
                if node.is_class() && ui.button(t!("libs-copy-class")).clicked() {
                    ui.ctx().copy_text(node.path.trim_end_matches(".class").replace('/', "."));
                }
                if ui.button(t!("libs-reveal-jar")).clicked() {
                    if let Some(l) = libs.libs.get(node.lib) {
                        cmds.push(Cmd::Reveal(l.jar.clone()));
                    }
                }
            });
        }
    }

    fn rows(&mut self, ui: &mut Ui, pal: &Palette, dir: &Path, depth: usize, cmds: &mut Vec<Cmd>) {
        for entry in self.children(dir) {
            if entry.hidden && !self.show_hidden {
                continue;
            }
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
            if resp.double_clicked() && !entry.dir {
                cmds.push(Cmd::OpenFile { path: entry.path.clone(), line: None });
            }
            crate::theme::context_menu(&resp, |ui| {
                if !entry.dir && ui.button(t!("project-open")).clicked() {
                    cmds.push(Cmd::OpenFile { path: entry.path.clone(), line: None });
                }
                if ui.button(t!("project-reveal")).clicked() {
                    cmds.push(Cmd::Reveal(entry.path.clone()));
                }
                if ui.button(t!("project-copy-path")).clicked() {
                    ui.ctx().copy_text(entry.path.display().to_string());
                }
                if let Some(rel) = self.root.as_deref().and_then(|r| entry.path.strip_prefix(r).ok()) {
                    if ui.button(t!("project-copy-relative")).clicked() {
                        ui.ctx().copy_text(rel.to_string_lossy().replace('\\', "/"));
                    }
                }
            });
            if open {
                self.rows(ui, pal, &entry.path, depth + 1, cmds);
            }
        }
    }
}

/// Сведения о моде под именем проекта: загрузчик, версии, id.
fn mod_info(ui: &mut Ui, pal: &Palette, project: &Project) {
    let info = &project.info;
    ui.horizontal_wrapped(|ui| {
        ui.add_space(10.0);
        ui.spacing_mut().item_spacing = vec2(5.0, 4.0);
        if !info.is_mod() {
            let text = if info.gradle { t!("project-not-a-mod") } else { t!("project-no-gradle") };
            ui.label(RichText::new(text).size(11.5).color(pal.text_faint));
            return;
        }
        for loader in &info.loaders {
            let origin = mycelium_project::origin::loader_origin(*loader);
            let text = match (&info.loader_version, info.is_multiloader()) {
                (Some(v), false) => format!("{} {v}", loader.name()),
                _ => loader.name().to_owned(),
            };
            chip(ui, pal, origin_color(pal, &origin), &text);
        }
        if let Some(mc) = &info.minecraft {
            chip(ui, pal, origin_color(pal, &mycelium_project::Origin::Minecraft), &format!("Minecraft {mc}"));
        }
        if let Some(id) = &info.mod_id {
            chip(ui, pal, pal.accent, id);
        }
    });
}

/// Служебные папки сборки показываем приглушённо.
fn is_noise(name: &str) -> bool {
    matches!(name, "build" | "run" | "runs" | "out" | "bin" | "gradle" | "target")
}

pub fn file_icon(name: &str, dir: bool, open: bool) -> (&'static str, Option<Color32>) {
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
    let (glyph, tint) = file_icon(&entry.name, entry.dir, open);
    let dim = entry.hidden || is_noise(&entry.name);
    let base = if dim { pal.text_faint } else { pal.text };
    let icon_color = tint.unwrap_or(pal.text_muted);
    let icon_color = if entry.hidden { icon_color.gamma_multiply(0.5) } else { icon_color };
    tree_row(ui, pal, depth, entry.dir.then_some(open), glyph, icon_color, &entry.name, base, None, selected)
}

/// Строка дерева: стрелка (если узел раскрывается), значок, текст и приглушённая подпись справа от него.
#[allow(clippy::too_many_arguments)]
fn tree_row(
    ui: &mut Ui,
    pal: &Palette,
    depth: usize,
    chevron: Option<bool>,
    glyph: &str,
    glyph_color: Color32,
    text: &str,
    text_color: Color32,
    suffix: Option<(&str, Color32)>,
    selected: bool,
) -> egui::Response {
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
    if let Some(open) = chevron {
        painter.text(pos2(x + 5.0, cy), Align2::CENTER_CENTER, if open { "▾" } else { "▸" }, FontId::proportional(12.0), pal.text_faint);
    }
    x += 14.0;
    painter.text(pos2(x + 7.0, cy), Align2::CENTER_CENTER, glyph, FontId::proportional(12.5), glyph_color);
    x += 20.0;
    let color = if selected { pal.heading } else { text_color.lerp_to_gamma(pal.heading, hover * 0.5) };
    let clipped = painter.with_clip_rect(rect);
    let r = clipped.text(pos2(x, cy), Align2::LEFT_CENTER, text, FontId::proportional(13.0), color);
    if let Some((s, c)) = suffix.filter(|(s, _)| !s.is_empty()) {
        clipped.text(pos2(r.right() + 8.0, cy), Align2::LEFT_CENTER, s, FontId::proportional(11.5), c);
    }
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

fn toggle(set: &mut HashSet<String>, key: &str) {
    if !set.remove(key) {
        set.insert(key.to_owned());
    }
}

fn read_dir_sorted(dir: &Path) -> Vec<Entry> {
    let Ok(read) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut entries: Vec<Entry> = read
        .flatten()
        .map(|e| {
            let path = e.path();
            let dir = path.is_dir();
            let hidden = store::is_hidden(&path);
            Entry { name: e.file_name().to_string_lossy().into_owned(), path, dir, hidden }
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
    fn folders_first_then_names_and_hidden_marked() {
        let dir = std::env::temp_dir().join(format!("mycelium-tree-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::create_dir_all(dir.join("Assets")).unwrap();
        std::fs::create_dir_all(dir.join(".mycelium")).unwrap();
        std::fs::write(dir.join("build.gradle"), "").unwrap();
        std::fs::write(dir.join("README.md"), "").unwrap();
        let entries = read_dir_sorted(&dir);
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec![".mycelium", "Assets", "src", "build.gradle", "README.md"]);
        assert!(entries[0].hidden && !entries[1].hidden);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn expanded_state_roundtrips_relative() {
        let root = PathBuf::from("C:/mods/test");
        let tree = ProjectTree::restore(&root, &["src/main".into(), "src".into()], true);
        assert_eq!(tree.expanded_relative(), vec!["src".to_owned(), "src/main".to_owned()]);
        assert!(tree.show_hidden);
    }
}
