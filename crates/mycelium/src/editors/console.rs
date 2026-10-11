//! Консоль: запуск клиента, сервера, генерации данных и сборки и их лог, разобранный
//! по серьёзности, системе игры и происхождению.

use std::collections::HashSet;

use eframe::egui::{self, Align, Align2, CornerRadius, CursorIcon, FontId, Layout, RichText, Sense, Ui, vec2};
use mycelium_i18n::t;
use mycelium_log::{Entry, Level};
use mycelium_project::Libraries;

use super::problem_ui::{self, entry_title, level_color, level_tag, origin_color, system_label};
use crate::app::Cmd;
use crate::run::{Console, RunConfig, Status};
use crate::theme::Palette;

const ROW_H: f32 = 21.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LevelFilter {
    All,
    Info,
    Warn,
    Error,
}

impl LevelFilter {
    fn min(self) -> Level {
        match self {
            LevelFilter::All => Level::Trace,
            LevelFilter::Info => Level::Info,
            LevelFilter::Warn => Level::Warn,
            LevelFilter::Error => Level::Error,
        }
    }
}

/// Состояние одной вкладки консоли: фильтры, выбор, раскрытые кадры.
pub struct ConsoleView {
    level: LevelFilter,
    only_problems: bool,
    only_own: bool,
    search: String,
    selected: Option<u64>,
    autoscroll: bool,
    expanded: HashSet<(u64, usize, usize)>,
    /// Отфильтрованные номера записей и ключ, для которого они посчитаны.
    cache: Option<(u64, (LevelFilter, bool, bool, String), Vec<usize>)>,
}

impl Default for ConsoleView {
    fn default() -> Self {
        Self {
            level: LevelFilter::All,
            only_problems: false,
            only_own: false,
            search: String::new(),
            selected: None,
            autoscroll: true,
            expanded: HashSet::new(),
            cache: None,
        }
    }
}

impl ConsoleView {
    fn filtered(&mut self, console: &Console) -> &[usize] {
        let key = (self.level, self.only_problems, self.only_own, self.search.to_lowercase());
        let fresh = matches!(&self.cache, Some((generation, k, _)) if *generation == console.generation && *k == key);
        if !fresh {
            let min = self.level.min();
            let query = key.3.clone();
            let rows = console
                .entries
                .iter()
                .enumerate()
                .filter(|(_, e)| e.level >= min || e.crash || (e.throwable.is_some() && min <= Level::Error))
                .filter(|(_, e)| !self.only_problems || (e.is_problem() && !e.noise))
                .filter(|(_, e)| !self.only_own || e.origin == mycelium_project::Origin::OwnMod)
                .filter(|(_, e)| query.is_empty() || e.search_text().to_lowercase().contains(&query))
                .map(|(i, _)| i)
                .collect();
            self.cache = Some((console.generation, key, rows));
        }
        &self.cache.as_ref().expect("посчитано выше").2
    }
}

/// Панель запуска: выбор конфигурации, «Запустить», «Остановить», состояние.
pub fn run_bar(ui: &mut Ui, pal: &Palette, console: &Console, configs: &[RunConfig], selected: &mut Option<String>, now: f64, show_status: bool, cmds: &mut Vec<Cmd>) {
    let current = configs.iter().find(|c| Some(&c.id) == selected.as_ref()).or(configs.first());
    let label = current.map(|c| format!("{}  {}", c.kind.icon(), c.label())).unwrap_or_else(|| t!("run-no-configs"));
    egui::ComboBox::from_id_salt("mycelium-run-config").selected_text(label).width(170.0).show_ui(ui, |ui| {
        for c in configs {
            let text = format!("{}  {}", c.kind.icon(), c.label());
            if ui.selectable_label(Some(&c.id) == selected.as_ref(), text).on_hover_text(format!("gradlew {}", c.task)).clicked() {
                *selected = Some(c.id.clone());
            }
        }
    });
    let running = console.is_running();
    if running {
        if ui.add(egui::Button::new(RichText::new("■").color(pal.danger))).on_hover_text(format!("{} (Ctrl+F2)", t!("run-stop"))).clicked() {
            cmds.push(Cmd::Stop);
        }
        if ui.button("⟲").on_hover_text(t!("run-restart")).clicked() {
            cmds.push(Cmd::Rerun);
        }
    } else if ui
        .add_enabled(current.is_some(), egui::Button::new(RichText::new("▶").color(pal.success)))
        .on_hover_text(format!("{} (Shift+F10)", t!("run-start")))
        .clicked()
    {
        cmds.push(Cmd::Run);
    }
    if show_status {
        status_label(ui, pal, console, now);
    }
}

fn status_label(ui: &mut Ui, pal: &Palette, console: &Console, now: f64) {
    let fmt = |s: f64| {
        let s = s.max(0.0) as u64;
        if s >= 60 { format!("{}:{:02}", s / 60, s % 60) } else { t!("run-seconds", count = s) }
    };
    let (text, color) = match &console.status {
        Status::Idle => return,
        Status::Running { since } => (t!("run-status-running", name = console.label.clone(), time = fmt(now - since)), pal.accent),
        Status::Finished { code: Some(0), seconds } => (t!("run-status-done", name = console.label.clone(), time = fmt(*seconds)), pal.success),
        Status::Finished { code, seconds } => (
            t!("run-status-failed", name = console.label.clone(), time = fmt(*seconds), code = code.map(|c| c.to_string()).unwrap_or_else(|| "?".into())),
            pal.danger,
        ),
        Status::Stopped { seconds } => (t!("run-status-stopped", name = console.label.clone(), time = fmt(*seconds)), pal.text_muted),
        Status::Failed(err) => (t!("run-status-error", error = err.clone()), pal.danger),
    };
    ui.label(RichText::new(text).size(12.5).color(color));
}

pub fn ui(
    ui: &mut Ui,
    pal: &Palette,
    view: &mut ConsoleView,
    console: &Console,
    libs: Option<&Libraries>,
    configs: &[RunConfig],
    selected_config: &mut Option<String>,
    has_project: bool,
    now: f64,
    cmds: &mut Vec<Cmd>,
) {
    if !has_project {
        super::empty_state(ui, pal, "🖥", &t!("console-no-project-title"), &t!("console-no-project-hint"));
        return;
    }
    // Панель: запуск слева, фильтры справа.
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        run_bar(ui, pal, console, configs, selected_config, now, true, cmds);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(6.0);
            if ui.button("🗑").on_hover_text(t!("console-clear")).clicked() {
                cmds.push(Cmd::ClearConsole);
                view.selected = None;
            }
            let auto = ui.add(egui::Button::new("⇩").selected(view.autoscroll)).on_hover_text(t!("console-autoscroll"));
            if auto.clicked() {
                view.autoscroll = !view.autoscroll;
            }
            if ui.add(egui::Button::new(t!("console-only-own")).selected(view.only_own)).clicked() {
                view.only_own = !view.only_own;
            }
            if ui.add(egui::Button::new(t!("console-only-problems")).selected(view.only_problems)).clicked() {
                view.only_problems = !view.only_problems;
            }
            for (f, label) in [(LevelFilter::Error, "✖"), (LevelFilter::Warn, "⚠"), (LevelFilter::Info, "ℹ"), (LevelFilter::All, "∗")].into_iter() {
                let hint = match f {
                    LevelFilter::All => t!("console-level-all"),
                    LevelFilter::Info => t!("console-level-info"),
                    LevelFilter::Warn => t!("console-level-warn"),
                    LevelFilter::Error => t!("console-level-error"),
                };
                if ui.add(egui::Button::new(label).selected(view.level == f)).on_hover_text(hint).clicked() {
                    view.level = f;
                }
            }
            ui.add(egui::TextEdit::singleline(&mut view.search).hint_text(t!("console-search")).desired_width(150.0));
        });
    });
    ui.add_space(3.0);

    let rows: Vec<usize> = view.filtered(console).to_vec();
    if rows.is_empty() {
        let (title, hint) = if console.entries.is_empty() {
            (t!("console-empty-title"), t!("console-empty-hint"))
        } else {
            (t!("console-filtered-title"), t!("console-filtered-hint"))
        };
        super::empty_state(ui, pal, "🖥", &title, &hint);
        return;
    }

    let selected_entry = view.selected.and_then(|seq| console.entries.iter().find(|e| e.seq == seq));
    let total_h = ui.available_height();
    let list_h = if selected_entry.is_some() { (total_h * 0.55).max(120.0) } else { total_h };

    // Список записей: одинаковая высота строк — рисуются только видимые.
    ui.allocate_ui(vec2(ui.available_width(), list_h), |ui| {
        let mut area = egui::ScrollArea::vertical().id_salt("mycelium-console-list").auto_shrink(false);
        if view.autoscroll {
            area = area.stick_to_bottom(true);
        }
        area.show_rows(ui, ROW_H, rows.len(), |ui, range| {
            for &i in &rows[range] {
                let e = &console.entries[i];
                if row(ui, pal, e, view.selected == Some(e.seq)).clicked() {
                    view.selected = if view.selected == Some(e.seq) { None } else { Some(e.seq) };
                    if view.selected.is_some() {
                        view.autoscroll = false;
                    }
                }
            }
        });
    });

    if let Some(e) = selected_entry {
        let (sep, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
        ui.painter().rect_filled(sep, CornerRadius::ZERO, pal.border);
        egui::ScrollArea::vertical().id_salt("mycelium-console-details").auto_shrink(false).show(ui, |ui| {
            ui.add_space(6.0);
            egui::Frame::new().inner_margin(egui::Margin::symmetric(10, 0)).show(ui, |ui| {
                problem_ui::entry_details(ui, pal, e, libs, &mut view.expanded, cmds);
                if e.is_problem() {
                    ui.add_space(6.0);
                    if ui.link(t!("console-show-in-exceptions")).clicked() {
                        cmds.push(Cmd::OpenEditor(super::EditorKind::Exceptions));
                    }
                }
            });
            ui.add_space(8.0);
        });
    }
}

/// Строка консоли: полоска происхождения, время, уровень, система, сообщение.
fn row(ui: &mut Ui, pal: &Palette, e: &Entry, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    let painter = ui.painter_at(rect);
    if selected {
        painter.rect_filled(rect, CornerRadius::same(4), pal.accent_soft);
    } else if resp.hovered() {
        painter.rect_filled(rect, CornerRadius::same(4), pal.bg_hover.gamma_multiply(0.6));
    }
    // Безобидный шум не подсвечиваем: он не требует внимания.
    if e.is_problem() && !e.noise {
        let tint = if e.level >= Level::Error || e.crash { pal.danger } else { pal.warning };
        if !selected {
            painter.rect_filled(rect, CornerRadius::ZERO, tint.gamma_multiply(0.06));
        }
    }
    painter.rect_filled(egui::Rect::from_min_size(rect.min + vec2(3.0, 3.0), vec2(3.0, ROW_H - 6.0)), CornerRadius::same(1), origin_color(pal, &e.origin));

    let mono = FontId::monospace(11.5);
    let mut x = rect.left() + 12.0;
    let cy = rect.center().y;
    let time = e.time.as_deref().map(short_time).unwrap_or_default();
    painter.text(egui::pos2(x, cy), Align2::LEFT_CENTER, time, mono.clone(), pal.text_faint);
    x += 62.0;
    let (tag, color) = if e.crash {
        ("CRASH", pal.danger)
    } else if e.noise {
        (level_tag(e.level), pal.text_faint)
    } else {
        (level_tag(e.level), level_color(pal, e.level))
    };
    painter.text(egui::pos2(x, cy), Align2::LEFT_CENTER, tag, mono.clone(), color);
    x += 48.0;
    if e.is_problem() {
        let label = if e.noise { t!("noise-tag") } else { system_label(e.system) };
        let sys = painter.text(egui::pos2(x, cy), Align2::LEFT_CENTER, label, FontId::proportional(11.5), pal.text_faint);
        x = sys.right() + 10.0;
    }
    let msg_color = if e.noise {
        pal.text_muted
    } else if e.crash || e.level >= Level::Error {
        pal.danger
    } else if e.level == Level::Warn {
        pal.warning
    } else if e.level <= Level::Debug {
        pal.text_faint
    } else {
        pal.text
    };
    let mut text = e.message.clone();
    if let Some(t) = &e.throwable {
        text.push_str("  ·  ");
        text.push_str(&entry_title_for_row(e, t));
    }
    if !e.details.is_empty() {
        text.push_str(&format!("  (+{})", e.details.len()));
    }
    painter.text(egui::pos2(x, cy), Align2::LEFT_CENTER, text, mono, msg_color);
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

fn entry_title_for_row(e: &Entry, t: &mycelium_log::Throwable) -> String {
    let root = t.root_cause();
    match &root.message {
        Some(m) if e.known.is_none() => format!("{}: {m}", root.short_type()),
        _ => entry_title(e),
    }
}

/// `11Oct2026 00:12:34.567` → `00:12:34`.
pub fn short_time(t: &str) -> String {
    let last = t.rsplit(' ').next().unwrap_or(t);
    last.split('.').next().unwrap_or(last).to_owned()
}
