//! Вкладка «Моды запуска»: всё, что попадёт в игру, — свой мод, Minecraft и загрузчик,
//! чужие моды (и вложенные в них jar), библиотеки. У мода — логотип, версия, откуда он
//! взялся, сколько у него проблем в запусках и каких обязательных зависимостей не хватает.
//! Моды разбирает Hyphae ([`mycelium_hyphae::RunMods`]).

use eframe::egui::{self, Align, CornerRadius, CursorIcon, FontId, Layout, RichText, Sense, Ui, vec2};
use forma_i18n::t;
use mycelium_hyphae::{Found, ModKind, RunMod, RunMods};
use mycelium_log::{Level, Problem};

use super::problem_ui;
use crate::app::Cmd;
use crate::run::Console;
use crate::sync::SyncState;
use forma_ui::theme::Palette;

/// id, которые не нужно искать среди модов: это сама игра и загрузчик.
const PLATFORM: &[&str] = &["minecraft", "neoforge", "forge", "fml", "fabricloader", "fabric", "java", "quilt_loader", "javafml"];

/// Состояние одной вкладки.
#[derive(Default)]
pub struct RunModsView {
    search: String,
    show_libraries: bool,
    selected: Option<String>,
}

impl RunModsView {
    /// Выбирает мод; поиск сбрасывается, чтобы мод точно был виден в списке.
    pub fn select(&mut self, id: &str) {
        self.selected = Some(id.to_owned());
        self.search.clear();
    }
}

/// Обязательная зависимость мода, которой нет в запуске.
struct Missing {
    owner: String,
    dep: String,
    versions: String,
}

fn missing_dependencies(run: &RunMods) -> Vec<Missing> {
    let mut out = Vec::new();
    for m in run.mods() {
        for d in m.dependencies.iter().filter(|d| d.required) {
            if PLATFORM.contains(&d.id.as_str()) || run.get(&d.id).is_some() {
                continue;
            }
            out.push(Missing { owner: m.id.clone(), dep: d.id.clone(), versions: d.versions.clone() });
        }
    }
    out
}

/// Проблемы запусков, владелец которых — этот мод (без безобидного шума).
fn problems_of<'a>(console: &'a Console, m: &RunMod) -> Vec<&'a Problem> {
    let origin = m.origin();
    console.problems.problems().iter().filter(|p| !p.sample.noise && p.sample.origin == origin).collect()
}

fn is_error(p: &Problem) -> bool {
    p.sample.crash || p.sample.level >= Level::Error || p.sample.throwable.is_some()
}

fn found_label(f: &Found) -> String {
    match f {
        Found::Project => t!("runmods-found-project"),
        Found::Classpath => t!("runmods-found-classpath"),
        Found::RunFolder => t!("runmods-found-run-folder"),
        Found::Nested { parent } => t!("runmods-found-nested", parent = crate::mods_view::mod_label(parent)),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn ui(ui: &mut Ui, pal: &Palette, view: &mut RunModsView, run: Option<&RunMods>, has_project: bool, console: &Console, sync: &SyncState, cmds: &mut Vec<Cmd>) {
    let Some(run) = run else {
        let (title, hint) = if !has_project {
            (t!("runmods-no-project-title"), t!("runmods-no-project-hint"))
        } else if matches!(sync, SyncState::Running { .. } | SyncState::Indexing) {
            (t!("runmods-loading"), t!("runmods-loading-hint"))
        } else {
            (t!("runmods-empty-title"), t!("runmods-empty-hint"))
        };
        super::empty_state(ui, pal, super::EditorKind::RunMods.icon(), &title, &hint);
        return;
    };

    let mods = run.mods().count();
    let libraries = run.mods.iter().filter(|m| m.kind == ModKind::Library).count();
    ui.add_space(4.0);
    let search_id = ui.id().with("mycelium-runmods-search");
    forma_ui::widgets::bar(
        ui,
        ui.id().with("mycelium-runmods-bar"),
        6.0,
        |ui| {
            let width = (ui.available_width() - 120.0).clamp(80.0, 240.0);
            forma_ui::widgets::search_field(ui, search_id, &mut view.search, &t!("runmods-search"), width, 12.5);
            if ui.add(egui::Button::new(t!("runmods-libraries")).selected(view.show_libraries)).clicked() {
                view.show_libraries = !view.show_libraries;
            }
        },
        |ui| {
            ui.label(RichText::new(t!("runmods-count", mods = mods, libs = libraries)).size(12.0).color(pal.text_faint));
        },
    );
    ui.add_space(4.0);

    // Что помешает игре запуститься: дубликаты и недостающие обязательные зависимости.
    let missing = missing_dependencies(run);
    if !run.duplicates.is_empty() || !missing.is_empty() {
        egui::Frame::new().inner_margin(egui::Margin::symmetric(10, 2)).show(ui, |ui| {
            for (id, place) in &run.duplicates {
                forma_ui::widgets::icon_label(ui, "error", t!("runmods-duplicate", id = crate::mods_view::mod_label(id), place = place.clone()), 12.0, pal.danger);
            }
            for m in &missing {
                let text = t!("runmods-missing-dep", owner = crate::mods_view::mod_label(&m.owner), dep = m.dep.clone(), versions = m.versions.clone());
                forma_ui::widgets::icon_label(ui, "warning", text, 12.0, pal.warning);
            }
        });
        ui.add_space(4.0);
    }

    let query = view.search.to_lowercase();
    let matches = |m: &RunMod| query.is_empty() || m.id.to_lowercase().contains(&query) || m.name.to_lowercase().contains(&query);
    let mut own: Vec<&RunMod> = Vec::new();
    let mut game: Vec<&RunMod> = Vec::new();
    let mut others: Vec<&RunMod> = Vec::new();
    let mut libs: Vec<&RunMod> = Vec::new();
    for m in run.mods.iter().filter(|m| matches(m)) {
        match m.kind {
            ModKind::Own => own.push(m),
            ModKind::Minecraft => game.push(m),
            // Части загрузчика без метаданных мода — те же библиотеки.
            ModKind::Loader(_) if m.format.is_none() && m.id != "neoforge" => libs.push(m),
            ModKind::Loader(_) => game.push(m),
            ModKind::Mod => others.push(m),
            ModKind::Library => libs.push(m),
        }
    }
    others.sort_by_key(|m| m.name.to_lowercase());
    libs.sort_by_key(|m| m.id.to_lowercase());
    let selected = view.selected.as_deref().and_then(|id| run.get(id));

    let wide = ui.available_width() > 720.0;
    let avail = ui.available_size();
    let list_size = if wide { vec2((avail.x * 0.45).max(300.0), avail.y) } else { vec2(avail.x, (avail.y * 0.55).max(160.0)) };
    let layout = if wide { Layout::left_to_right(Align::Min) } else { Layout::top_down(Align::Min) };
    ui.with_layout(layout, |ui| {
        ui.allocate_ui_with_layout(list_size, Layout::top_down_justified(Align::Min), |ui| {
            egui::ScrollArea::vertical().id_salt("mycelium-runmods-list").auto_shrink(false).show(ui, |ui| {
                let sections: [(String, &Vec<&RunMod>, bool); 4] = [
                    (t!("runmods-section-own"), &own, true),
                    (t!("runmods-section-game"), &game, true),
                    (t!("runmods-section-mods", count = others.len()), &others, true),
                    (t!("runmods-section-libraries", count = libs.len()), &libs, view.show_libraries || !query.is_empty()),
                ];
                for (title, list, visible) in sections {
                    if list.is_empty() || !visible {
                        continue;
                    }
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        ui.label(RichText::new(title).size(12.0).family(forma_ui::fonts::semibold()).color(pal.text_faint));
                    });
                    for m in list {
                        let is_selected = view.selected.as_deref() == Some(m.id.as_str());
                        if mod_row(ui, pal, m, console, is_selected).clicked() {
                            view.selected = if is_selected { None } else { Some(m.id.clone()) };
                        }
                    }
                }
                ui.add_space(8.0);
            });
        });
        let (sep, _) = ui.allocate_exact_size(if wide { vec2(1.0, avail.y) } else { vec2(avail.x, 1.0) }, Sense::hover());
        ui.painter().rect_filled(sep, CornerRadius::ZERO, pal.border);
        ui.with_layout(Layout::top_down(Align::Min), |ui| {
            egui::ScrollArea::vertical().id_salt("mycelium-runmods-details").auto_shrink(false).show(ui, |ui| {
                egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 8)).show(ui, |ui| match selected {
                    Some(m) => details(ui, pal, m, run, console, cmds),
                    None => {
                        ui.add_space(20.0);
                        ui.label(RichText::new(t!("runmods-pick")).color(pal.text_faint));
                    }
                });
            });
        });
    });
}

/// Логотип мода, а если его нет — значок вида.
fn paint_logo(ui: &Ui, rect: egui::Rect, m: &RunMod, pal: &Palette) {
    match crate::mods_view::logo(&m.id) {
        Some(tex) => {
            ui.painter().image(tex.id(), rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
        }
        None => {
            let icon = match m.kind {
                ModKind::Library | ModKind::Loader(_) => "jar",
                ModKind::Minecraft => "package",
                _ => "plugin",
            };
            forma_ui::icons::paint(ui.painter(), rect.shrink(rect.width() * 0.18), icon, problem_ui::origin_color(pal, &m.origin()));
        }
    }
}

fn mod_row(ui: &mut Ui, pal: &Palette, m: &RunMod, console: &Console, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 42.0), Sense::click());
    let painter = ui.painter_at(rect);
    if selected {
        painter.rect_filled(rect.shrink2(vec2(4.0, 1.0)), CornerRadius::same(6), pal.list_selected);
    } else if resp.hovered() {
        painter.rect_filled(rect.shrink2(vec2(4.0, 1.0)), CornerRadius::same(6), pal.list_hover);
    }
    let logo = egui::Rect::from_min_size(rect.min + vec2(12.0, 7.0), vec2(28.0, 28.0));
    paint_logo(ui, logo, m, pal);
    let x = logo.right() + 10.0;
    let right = rect.right() - 10.0;

    // Счётчики проблем мода — справа, не обрезаются.
    let problems = problems_of(console, m);
    let errors = problems.iter().filter(|p| is_error(p)).count();
    let warnings = problems.len() - errors;
    let mut counts_w = 0.0;
    let mut cx = right;
    for (icon, n, color) in [("warning", warnings, pal.warning), ("error", errors, pal.danger)] {
        if n == 0 {
            continue;
        }
        let g = painter.layout_no_wrap(n.to_string(), FontId::proportional(12.0), color);
        cx -= g.size().x;
        let gw = g.size().x;
        painter.galley(egui::pos2(cx, rect.top() + 13.0 - g.size().y / 2.0), g, color);
        cx -= 15.0;
        forma_ui::icons::paint(&painter, egui::Rect::from_center_size(egui::pos2(cx + 6.5, rect.top() + 13.0), vec2(13.0, 13.0)), icon, color);
        cx -= 8.0;
        counts_w += gw + 23.0;
    }

    let name_color = if m.kind == ModKind::Own { crate::colors::current().tag_own } else { pal.heading };
    let version = painter.layout_no_wrap(m.version.clone(), FontId::proportional(11.5), pal.text_faint);
    let name_room = right - counts_w - x - if m.version.is_empty() { 0.0 } else { version.size().x + 8.0 };
    let name = forma_ui::widgets::elide(ui, m.name.clone(), FontId::proportional(13.0), name_color, name_room.max(30.0));
    let name_w = name.size().x;
    painter.galley(egui::pos2(x, rect.top() + 13.0 - name.size().y / 2.0), name, name_color);
    if !m.version.is_empty() && name_room > 30.0 {
        painter.galley(egui::pos2(x + name_w + 8.0, rect.top() + 13.0 - version.size().y / 2.0), version, pal.text_faint);
    }
    let sub = format!("{} · {}", m.id, found_label(&m.found));
    let sub = forma_ui::widgets::elide(ui, sub, FontId::proportional(11.5), pal.text_faint, right - x);
    painter.galley(egui::pos2(x, rect.top() + 29.0 - sub.size().y / 2.0), sub, pal.text_faint);
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// Строка «Подпись: значение» в подробностях.
fn fact(ui: &mut Ui, pal: &Palette, label: &str, value: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(label).size(12.0).color(pal.text_faint));
        ui.add(egui::Label::new(RichText::new(value).size(12.0).color(pal.text)).wrap());
    });
}

fn details(ui: &mut Ui, pal: &Palette, m: &RunMod, run: &RunMods, console: &Console, cmds: &mut Vec<Cmd>) {
    ui.horizontal(|ui| {
        let (logo, _) = ui.allocate_exact_size(vec2(48.0, 48.0), Sense::hover());
        paint_logo(ui, logo, m, pal);
        ui.add_space(6.0);
        ui.vertical(|ui| {
            ui.add(egui::Label::new(RichText::new(&m.name).size(17.0).family(forma_ui::fonts::semibold()).color(pal.heading)).wrap());
            let mut line = vec![m.id.clone()];
            if !m.version.is_empty() {
                line.push(m.version.clone());
            }
            line.push(found_label(&m.found));
            ui.label(RichText::new(line.join(" · ")).size(12.0).color(pal.text_faint));
        });
    });
    ui.add_space(8.0);
    if !m.description.is_empty() {
        ui.add(egui::Label::new(RichText::new(m.description.trim()).size(13.0).color(pal.text)).wrap());
        ui.add_space(6.0);
    }
    if !m.authors.is_empty() {
        fact(ui, pal, &t!("runmods-authors"), &m.authors);
    }
    if let Some(jar) = &m.jar {
        let place = match &m.inner {
            Some(inner) => format!("{} › {inner}", jar.display()),
            None => jar.display().to_string(),
        };
        fact(ui, pal, &t!("runmods-file"), &place);
    }
    if let Some(module) = &m.module {
        fact(ui, pal, &t!("runmods-module"), module);
    }
    if !m.packages.is_empty() {
        fact(ui, pal, &t!("runmods-packages"), &m.packages.len().to_string());
    }

    if !m.dependencies.is_empty() {
        ui.add_space(8.0);
        ui.label(RichText::new(t!("runmods-dependencies")).size(13.0).family(forma_ui::fonts::semibold()).color(pal.heading));
        for d in &m.dependencies {
            let present = PLATFORM.contains(&d.id.as_str()) || run.get(&d.id).is_some();
            let (icon, color) = match (present, d.required) {
                (true, _) => ("success", pal.success),
                (false, true) => ("error", pal.danger),
                (false, false) => ("info", pal.text_faint),
            };
            let mut text = crate::mods_view::mod_label(&d.id);
            if !d.versions.is_empty() {
                text.push_str(&format!("  {}", d.versions));
            }
            if !d.required {
                text.push_str(&format!(" · {}", t!("runmods-optional")));
            }
            if !present && d.required {
                text.push_str(&format!(" · {}", t!("runmods-not-in-run")));
            }
            forma_ui::widgets::icon_label(ui, icon, text, 12.5, color);
        }
    }

    if !m.mixins.is_empty() {
        ui.add_space(8.0);
        ui.label(RichText::new(t!("runmods-mixins")).size(13.0).family(forma_ui::fonts::semibold()).color(pal.heading));
        for c in &m.mixins {
            ui.label(RichText::new(c).font(FontId::monospace(12.0)).color(pal.text_muted));
        }
    }

    ui.add_space(8.0);
    ui.label(RichText::new(t!("runmods-problems")).size(13.0).family(forma_ui::fonts::semibold()).color(pal.heading));
    let problems = problems_of(console, m);
    if problems.is_empty() {
        ui.label(RichText::new(t!("runmods-no-problems")).size(12.5).color(pal.text_faint));
    }
    // Щелчок по проблеме открывает её в окне исключений.
    for p in problems.iter().take(30) {
        let (icon, color) = if is_error(p) { ("error", pal.danger) } else { ("warning", pal.warning) };
        let title = problem_ui::entry_title(&p.sample);
        let text = if p.count > 1 { format!("{title}  ×{}", p.count) } else { title };
        let resp = forma_ui::widgets::icon_label(ui, icon, text, 12.5, color).interact(Sense::click());
        if resp.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(t!("runmods-open-problem")).clicked() {
            cmds.push(Cmd::ShowProblem(p.signature.clone()));
        }
    }
}
