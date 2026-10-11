//! Окно исключений: все проблемы запусков, сгруппированные по системам игры, типам,
//! происхождению или серьёзности, со счётчиками повторов и объяснениями простыми словами.

use std::collections::{BTreeMap, HashSet};

use eframe::egui::{self, Align, Align2, CornerRadius, CursorIcon, FontId, Layout, RichText, Sense, Ui, vec2};
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

fn severity_glyph(p: &Problem) -> &'static str {
    match severity(p) {
        0 => "💥",
        1 => "✖",
        _ => "⚠",
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

    // Панель: группировка, фильтры, счётчики.
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        let labels = [t!("exceptions-by-system"), t!("exceptions-by-type"), t!("exceptions-by-origin"), t!("exceptions-by-severity")];
        let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let modes = [GroupBy::System, GroupBy::Type, GroupBy::Origin, GroupBy::Severity];
        let current = modes.iter().position(|m| *m == view.group_by).unwrap_or(0);
        if let Some(i) = forma_ui::widgets::segmented(ui, pal, egui::Id::new("mycelium-exceptions-group"), &refs, current, false) {
            view.group_by = modes[i];
        }
        ui.add_space(6.0);
        if ui.add(egui::Button::new(t!("console-only-own")).selected(view.only_own)).clicked() {
            view.only_own = !view.only_own;
        }
        if ui.add(egui::Button::new(t!("exceptions-only-errors")).selected(view.only_errors)).clicked() {
            view.only_errors = !view.only_errors;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(6.0);
            if ui.button("🗑").on_hover_text(t!("exceptions-clear")).clicked() {
                cmds.push(Cmd::ClearProblems);
                view.selected = None;
            }
            ui.label(RichText::new(format!("⚠ {warnings}")).color(pal.warning));
            ui.label(RichText::new(format!("✖ {errors}")).color(pal.danger));
        });
    });
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
        super::empty_state(ui, pal, "⚠", &title, &hint);
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
                        ui.label(RichText::new(format!("  ✔ {}", t!("exceptions-only-noise"))).color(pal.success));
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
    painter.text(rect.left_center() + vec2(10.0, 0.0), Align2::LEFT_CENTER, if collapsed { "▸" } else { "▾" }, FontId::proportional(12.0), pal.text_faint);
    let name = painter.text(rect.left_center() + vec2(26.0, 0.0), Align2::LEFT_CENTER, label, FontId::new(13.0, forma_ui::fonts::semibold()), pal.heading);
    let counts = if total as usize > problems { format!("{problems} · ×{total}") } else { problems.to_string() };
    painter.text(egui::pos2(name.right() + 8.0, rect.center().y), Align2::LEFT_CENTER, counts, FontId::proportional(12.0), pal.text_faint);
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

fn problem_row(ui: &mut Ui, pal: &Palette, p: &Problem, run: u32, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::click());
    let painter = ui.painter_at(rect);
    if selected {
        painter.rect_filled(rect.shrink2(vec2(4.0, 1.0)), CornerRadius::same(6), pal.accent_soft);
    } else if resp.hovered() {
        painter.rect_filled(rect.shrink2(vec2(4.0, 1.0)), CornerRadius::same(6), pal.bg_hover.gamma_multiply(0.6));
    }
    let e = &p.sample;
    painter.rect_filled(egui::Rect::from_min_size(rect.min + vec2(8.0, 7.0), vec2(3.0, rect.height() - 14.0)), CornerRadius::same(1), origin_color(pal, &e.origin));
    let sev_color = match severity(p) {
        _ if e.noise => pal.text_faint,
        0 | 1 => pal.danger,
        _ => pal.warning,
    };
    painter.text(egui::pos2(rect.left() + 26.0, rect.top() + 12.0), Align2::CENTER_CENTER, severity_glyph(p), FontId::proportional(12.5), sev_color);
    let title = entry_title(e);
    let title_color = if e.noise { pal.text_muted } else { pal.heading };
    let t_rect = painter.text(egui::pos2(rect.left() + 38.0, rect.top() + 12.0), Align2::LEFT_CENTER, &title, FontId::proportional(13.0), title_color);
    let mut x = t_rect.right() + 8.0;
    if p.count > 1 {
        let r = painter.text(egui::pos2(x, rect.top() + 12.0), Align2::LEFT_CENTER, format!("×{}", p.count), FontId::proportional(12.0), pal.text_muted);
        x = r.right() + 8.0;
    }
    if p.is_new(run) {
        let g = painter.layout_no_wrap(t!("exceptions-new"), FontId::proportional(10.5), pal.accent);
        let r = egui::Rect::from_min_size(egui::pos2(x, rect.top() + 5.0), g.size() + vec2(10.0, 4.0));
        painter.rect_filled(r, CornerRadius::same(7), pal.accent_soft);
        painter.galley(r.min + vec2(5.0, 2.0), g, pal.accent);
    }
    // Вторая строка: сообщение и происхождение.
    let sub = if e.known.is_some() || e.throwable.is_none() { e.message.clone() } else { e.throwable.as_ref().and_then(|t| t.root_cause().message.clone()).unwrap_or_else(|| e.message.clone()) };
    let sub = format!("{} · {}", origin_label(&e.origin), sub);
    painter.text(egui::pos2(rect.left() + 38.0, rect.top() + 27.0), Align2::LEFT_CENTER, sub, FontId::proportional(11.5), pal.text_faint);
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
