//! Окно исключений: все проблемы запусков, сгруппированные по системам игры, типам,
//! происхождению или серьёзности, со счётчиками повторов и объяснениями простыми словами.

use std::collections::{BTreeMap, HashSet};

use eframe::egui::{self, Align, CornerRadius, CursorIcon, FontId, Layout, RichText, Sense, Ui, vec2};
use forma_i18n::t;
use mycelium_log::{Level, Problem};
use mycelium_project::{Libraries, Origin};

use super::problem_ui::{self, entry_title, origin_color, origin_label, system_label};
use crate::app::Cmd;
use crate::run::Console;
use forma_ui::theme::Palette;

#[derive(Clone, Copy, PartialEq, Eq)]
enum GroupBy {
    System,
    Type,
    Origin,
    Severity,
}

/// Состояние одной вкладки «Исключения».
pub struct ExceptionsView {
    group_by: GroupBy,
    only_own: bool,
    only_errors: bool,
    selected: Option<String>,
    collapsed: HashSet<String>,
    /// Группа безобидного шума раскрыта (по умолчанию свёрнута).
    noise_open: bool,
    expanded: HashSet<(u64, usize, usize)>,
}

impl Default for ExceptionsView {
    fn default() -> Self {
        Self {
            group_by: GroupBy::System,
            only_own: false,
            only_errors: false,
            selected: None,
            collapsed: HashSet::new(),
            noise_open: false,
            expanded: HashSet::new(),
        }
    }
}

impl ExceptionsView {
    /// Выбирает проблему и раскрывает группы, чтобы она была видна в списке.
    pub fn select(&mut self, signature: &str) {
        self.selected = Some(signature.to_owned());
        self.collapsed.clear();
        // Фильтры могли бы спрятать выбранную проблему.
        self.only_own = false;
        self.only_errors = false;
    }
}

/// Серьёзность для сортировки: краш, ошибка, предупреждение.
fn severity(p: &Problem) -> u8 {
    if p.sample.crash {
        0
    } else if p.sample.level >= Level::Error || p.sample.throwable.is_some() {
        1
    } else {
        2
    }
}

/// Значок серьёзности: краш, ошибка, предупреждение.
fn severity_icon(p: &Problem) -> &'static str {
    match severity(p) {
        0 => "crash",
        1 => "error",
        _ => "warning",
    }
}

/// Ключ и подпись группы для проблемы. Ключ задаёт порядок групп.
fn group_of(p: &Problem, by: GroupBy) -> (String, String) {
    let e = &p.sample;
    match by {
        GroupBy::System => (format!("{:02}", e.system as u8), system_label(e.system)),
        GroupBy::Type => {
            let title = match (&e.known, &e.throwable) {
                (Some(k), _) => problem_ui::known_title(*k),
                (None, Some(t)) => t.root_cause().short_type().to_owned(),
                (None, None) if e.level >= Level::Error => t!("exceptions-type-error"),
                _ => t!("exceptions-type-warning"),
            };
            (title.clone(), title)
        }
        GroupBy::Origin => {
            let order = match &e.origin {
                Origin::OwnMod => 0,
                Origin::OtherMod(_) => 1,
                Origin::NeoForge | Origin::Fabric | Origin::Forge => 2,
                Origin::Minecraft => 3,
                Origin::Library | Origin::Java => 4,
                Origin::Unknown => 5,
            };
            (format!("{order}{}", origin_label(&e.origin)), origin_label(&e.origin))
        }
        GroupBy::Severity => {
            let s = severity(p);
            let label = match s {
                0 => t!("exceptions-severity-crash"),
                1 => t!("exceptions-severity-error"),
                _ => t!("exceptions-severity-warning"),
            };
            (s.to_string(), label)
        }
    }
}

pub fn ui(ui: &mut Ui, pal: &Palette, view: &mut ExceptionsView, console: &Console, libs: Option<&Libraries>, cmds: &mut Vec<Cmd>) {
    let run = console.problems.run();
    let all = console.problems.problems();
    // Шум в счётчики не входит: он есть у любого мода.
    let errors = all.iter().filter(|p| !p.sample.noise && severity(p) <= 1).count();
    let warnings = all.iter().filter(|p| !p.sample.noise).count() - errors;

    // Панель: группировка и фильтры слева, счётчики справа. В узкой области счётчики
    // уходят на вторую строку, группировка становится выпадающим списком, а фильтры
    // сворачиваются в меню «⋯».
    ui.add_space(4.0);
    let mut cleared = false;
    let bar_id = ui.id().with("mycelium-exceptions-bar");
    let (only_own, only_errors) = (t!("console-only-own"), t!("exceptions-only-errors"));
    let toggles_w = forma_ui::widgets::button_width(ui, &only_own, 0.0) + forma_ui::widgets::button_width(ui, &only_errors, 0.0) + 22.0;
    forma_ui::widgets::bar(
        ui,
        bar_id,
        6.0,
        |ui| {
            let labels = [t!("exceptions-by-system"), t!("exceptions-by-type"), t!("exceptions-by-origin"), t!("exceptions-by-severity")];
            let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
            let modes = [GroupBy::System, GroupBy::Type, GroupBy::Origin, GroupBy::Severity];
            let current = modes.iter().position(|m| *m == view.group_by).unwrap_or(0);
            let segmented_w = forma_ui::widgets::segmented_width(ui, &refs);
            let compact = segmented_w + toggles_w > ui.available_width();
            if segmented_w <= ui.available_width() {
                if let Some(i) = forma_ui::widgets::segmented(ui, pal, bar_id.with("group-by"), &refs, current, false) {
                    view.group_by = modes[i];
                }
            } else {
                let options: Vec<(GroupBy, String)> = modes.into_iter().zip(labels).collect();
                let width = (ui.available_width() - 40.0).clamp(90.0, 160.0);
                if let Some(m) = forma_ui::widgets::dropdown(ui, pal, bar_id.with("group"), width, &view.group_by, &options) {
                    view.group_by = m;
                }
            }
            if compact {
                let more = forma_ui::icons::button(ui, "filter", view.only_own || view.only_errors).on_hover_text(t!("console-filters"));
                egui::Popup::menu(&more).style(forma_ui::theme::menu_style).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                    if ui.add(egui::Button::new(&only_own).selected(view.only_own)).clicked() {
                        view.only_own = !view.only_own;
                    }
                    if ui.add(egui::Button::new(&only_errors).selected(view.only_errors)).clicked() {
                        view.only_errors = !view.only_errors;
                    }
                });
            } else {
                ui.add_space(6.0);
                if ui.add(egui::Button::new(&only_own).selected(view.only_own)).clicked() {
                    view.only_own = !view.only_own;
                }
                if ui.add(egui::Button::new(&only_errors).selected(view.only_errors)).clicked() {
                    view.only_errors = !view.only_errors;
                }
            }
        },
        |ui| {
            if forma_ui::icons::button(ui, "trash", false).on_hover_text(t!("exceptions-clear")).clicked() {
                cmds.push(Cmd::ClearProblems);
                cleared = true;
            }
            super::problem_ui::icon_count(ui, "warning", &warnings.to_string(), 13.0, pal.warning);
            super::problem_ui::icon_count(ui, "error", &errors.to_string(), 13.0, pal.danger);
        },
    );
    if cleared {
        view.selected = None;
    }
    ui.add_space(3.0);

    let (noise, visible): (Vec<&Problem>, Vec<&Problem>) = all
        .iter()
        .filter(|p| !view.only_own || p.sample.origin == Origin::OwnMod)
        .filter(|p| !view.only_errors || severity(p) <= 1)
        .partition(|p| p.sample.noise);
    if visible.is_empty() && noise.is_empty() {
        let (title, hint) = if all.is_empty() {
            (t!("exceptions-empty-title"), t!("exceptions-empty-hint"))
        } else {
            (t!("console-filtered-title"), t!("console-filtered-hint"))
        };
        super::empty_state(ui, pal, "editor-exceptions", &title, &hint);
        return;
    }

    // Группы по выбранному признаку, внутри — сначала серьёзные и частые.
    let mut groups: BTreeMap<String, (String, Vec<&Problem>)> = BTreeMap::new();
    for &p in &visible {
        let (key, label) = group_of(p, view.group_by);
        groups.entry(key).or_insert_with(|| (label, Vec::new())).1.push(p);
    }
    for (_, list) in groups.values_mut() {
        list.sort_by(|a, b| severity(a).cmp(&severity(b)).then(b.count.cmp(&a.count)));
    }
    let selected = view.selected.as_ref().and_then(|s| all.iter().find(|p| &p.signature == s));

    // Широкая область — список слева и подробности справа; узкая — друг над другом.
    let wide = ui.available_width() > 720.0;
    let avail = ui.available_size();
    let list_size = if wide { vec2((avail.x * 0.42).max(280.0), avail.y) } else { vec2(avail.x, (avail.y * 0.5).max(140.0)) };
    let layout = if wide { Layout::left_to_right(Align::Min) } else { Layout::top_down(Align::Min) };
    ui.with_layout(layout, |ui| {
        // Внутри списка — сверху вниз, иначе строки унаследуют раскладку «слева направо».
        ui.allocate_ui_with_layout(list_size, Layout::top_down_justified(Align::Min), |ui| {
            egui::ScrollArea::vertical().id_salt("mycelium-exceptions-list").auto_shrink(false).show(ui, |ui| {
                for (key, (label, list)) in &groups {
                    let collapsed = view.collapsed.contains(key);
                    if group_header(ui, pal, label, list.len(), list.iter().map(|p| p.count).sum(), collapsed).clicked() {
                        if !view.collapsed.remove(key) {
                            view.collapsed.insert(key.clone());
                        }
                    }
                    if collapsed {
                        continue;
                    }
                    for p in list {
                        let is_selected = view.selected.as_ref() == Some(&p.signature);
                        if problem_row(ui, pal, p, run, is_selected).clicked() {
                            view.selected = if is_selected { None } else { Some(p.signature.clone()) };
                        }
                    }
                }
                // Безобидный шум — отдельной свёрнутой группой в конце, при любой группировке.
                if !noise.is_empty() {
                    if visible.is_empty() {
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.add_space(8.0);
                            forma_ui::widgets::icon_label(ui, "success", t!("exceptions-only-noise"), 13.0, pal.success);
                        });
                        ui.add_space(4.0);
                    }
                    let total = noise.iter().map(|p| p.count).sum();
                    let header = group_header(ui, pal, &t!("exceptions-noise-group"), noise.len(), total, !view.noise_open);
                    if header.on_hover_text(t!("exceptions-noise-hint")).clicked() {
                        view.noise_open = !view.noise_open;
                    }
                    if view.noise_open {
                        for p in &noise {
                            let is_selected = view.selected.as_ref() == Some(&p.signature);
                            if problem_row(ui, pal, p, run, is_selected).clicked() {
                                view.selected = if is_selected { None } else { Some(p.signature.clone()) };
                            }
                        }
                    }
                }
                ui.add_space(8.0);
            });
        });
        let (sep, _) = ui.allocate_exact_size(if wide { vec2(1.0, avail.y) } else { vec2(avail.x, 1.0) }, Sense::hover());
        ui.painter().rect_filled(sep, CornerRadius::ZERO, pal.border);
        ui.with_layout(Layout::top_down(Align::Min), |ui| {
            egui::ScrollArea::vertical().id_salt("mycelium-exceptions-details").auto_shrink(false).show(ui, |ui| {
                egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 8)).show(ui, |ui| match selected {
                    Some(p) => details(ui, pal, p, run, libs, &mut view.expanded, cmds),
                    None => {
                        ui.add_space(20.0);
                        ui.label(RichText::new(t!("exceptions-pick")).color(pal.text_faint));
                    }
                });
            });
        });
    });
}

fn group_header(ui: &mut Ui, pal: &Palette, label: &str, problems: usize, total: u32, collapsed: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(5), pal.bg_hover.gamma_multiply(0.6));
    }
    let painter = ui.painter_at(rect);
    let arrow = egui::Rect::from_center_size(rect.left_center() + vec2(15.0, 0.0), vec2(12.0, 12.0));
    forma_ui::icons::paint(&painter, arrow, if collapsed { "chevron-right" } else { "chevron-down" }, pal.text_faint);
    let counts = if total as usize > problems { format!("{problems} · ×{total}") } else { problems.to_string() };
    let counts = painter.layout_no_wrap(counts, FontId::proportional(12.0), pal.text_faint);
    // Название группы обрезается, а счётчик остаётся виден.
    let name = forma_ui::widgets::elide(ui, label, FontId::new(13.0, forma_ui::fonts::semibold()), pal.heading, rect.width() - 26.0 - 8.0 - counts.size().x - 6.0);
    let name_w = name.size().x;
    painter.galley(egui::pos2(rect.left() + 26.0, rect.center().y - name.size().y / 2.0), name, pal.heading);
    painter.galley(egui::pos2(rect.left() + 26.0 + name_w + 8.0, rect.center().y - counts.size().y / 2.0), counts, pal.text_faint);
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

fn problem_row(ui: &mut Ui, pal: &Palette, p: &Problem, run: u32, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::click());
    let painter = ui.painter_at(rect);
    if selected {
        painter.rect_filled(rect.shrink2(vec2(4.0, 1.0)), CornerRadius::same(6), pal.list_selected);
    } else if resp.hovered() {
        painter.rect_filled(rect.shrink2(vec2(4.0, 1.0)), CornerRadius::same(6), pal.list_hover);
    }
    let e = &p.sample;
    painter.rect_filled(egui::Rect::from_min_size(rect.min + vec2(8.0, 7.0), vec2(3.0, rect.height() - 14.0)), CornerRadius::same(1), origin_color(pal, &e.origin));
    let sev_color = match severity(p) {
        _ if e.noise => pal.text_faint,
        0 | 1 => pal.danger,
        _ => pal.warning,
    };
    forma_ui::icons::paint(&painter, egui::Rect::from_center_size(egui::pos2(rect.left() + 26.0, rect.top() + 12.0), vec2(14.0, 14.0)), severity_icon(p), sev_color);
    let title = entry_title(e);
    let title_color = if e.noise { pal.text_muted } else { pal.heading };
    // Счётчик повторов и отметка «новое» не обрезаются — место под них берётся у названия.
    let count = (p.count > 1).then(|| painter.layout_no_wrap(format!("×{}", p.count), FontId::proportional(12.0), pal.text_muted));
    let badge = p.is_new(run).then(|| painter.layout_no_wrap(t!("exceptions-new"), FontId::proportional(10.5), pal.accent));
    let extras = count.as_ref().map_or(0.0, |g| g.size().x + 8.0) + badge.as_ref().map_or(0.0, |g| g.size().x + 18.0);
    let right = rect.right() - 8.0;
    let title = forma_ui::widgets::elide(ui, title, FontId::proportional(13.0), title_color, right - (rect.left() + 38.0) - extras);
    let mut x = rect.left() + 38.0 + title.size().x + 8.0;
    painter.galley(egui::pos2(rect.left() + 38.0, rect.top() + 12.0 - title.size().y / 2.0), title, title_color);
    if let Some(g) = count {
        let w = g.size().x;
        painter.galley(egui::pos2(x, rect.top() + 12.0 - g.size().y / 2.0), g, pal.text_muted);
        x += w + 8.0;
    }
    if let Some(g) = badge {
        let r = egui::Rect::from_min_size(egui::pos2(x, rect.top() + 5.0), g.size() + vec2(10.0, 4.0));
        painter.rect_filled(r, CornerRadius::same(7), pal.accent_soft);
        painter.galley(r.min + vec2(5.0, 2.0), g, pal.accent);
    }
    // Вторая строка: сообщение и происхождение.
    let sub = if e.known.is_some() || e.throwable.is_none() { e.message.clone() } else { e.throwable.as_ref().and_then(|t| t.root_cause().message.clone()).unwrap_or_else(|| e.message.clone()) };
    let sub = format!("{} · {}", origin_label(&e.origin), sub.lines().next().unwrap_or(""));
    let sub = forma_ui::widgets::elide(ui, sub, FontId::proportional(11.5), pal.text_faint, right - (rect.left() + 38.0));
    painter.galley(egui::pos2(rect.left() + 38.0, rect.top() + 27.0 - sub.size().y / 2.0), sub, pal.text_faint);
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

fn details(ui: &mut Ui, pal: &Palette, p: &Problem, run: u32, libs: Option<&Libraries>, expanded: &mut HashSet<(u64, usize, usize)>, cmds: &mut Vec<Cmd>) {
    ui.label(RichText::new(entry_title(&p.sample)).size(17.0).family(forma_ui::fonts::semibold()).color(pal.heading));
    let mut facts = vec![t!("exceptions-count", count = p.count)];
    if let (Some(first), Some(last)) = (&p.first_time, &p.last_time) {
        let (first, last) = (super::console::short_time(first), super::console::short_time(last));
        facts.push(if first == last { first } else { format!("{first} – {last}") });
    }
    if p.is_new(run) {
        facts.push(t!("exceptions-new-in-run"));
    } else if p.last_run < run {
        facts.push(t!("exceptions-not-in-last-run"));
    }
    ui.label(RichText::new(facts.join(" · ")).size(12.0).color(pal.text_faint));
    ui.add_space(8.0);
    problem_ui::entry_details(ui, pal, &p.sample, libs, expanded, cmds);
}
