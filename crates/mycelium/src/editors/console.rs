//! Консоль: запуск клиента, сервера, генерации данных, сборки и своих задач и их лог,
//! разобранный по серьёзности, системе игры и происхождению. Несколько запусков могут
//! идти одновременно — у каждого своя вкладка.

use std::collections::HashSet;

use eframe::egui::{self, Align, Align2, CornerRadius, CursorIcon, FontId, Layout, RichText, Sense, Stroke, Ui, vec2};
use mycelium_i18n::t;
use mycelium_log::{Entry, Level};
use mycelium_project::Libraries;

use super::problem_ui::{self, entry_title, level_color, level_tag, origin_color, system_label};
use crate::app::Cmd;
use crate::run::{Console, RunConfig, Session, Status};
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

type FilterKey = (u64, LevelFilter, bool, bool, String);

/// Состояние одной вкладки консоли: какой запуск показан, фильтры, выбор, раскрытые кадры.
pub struct ConsoleView {
    /// Показанный запуск.
    session: Option<u64>,
    /// Последний запуск, на который консоль уже переключалась сама.
    followed: u64,
    level: LevelFilter,
    only_problems: bool,
    only_own: bool,
    search: String,
    selected: Option<u64>,
    autoscroll: bool,
    expanded: HashSet<(u64, usize, usize)>,
    /// Отфильтрованные номера записей и ключ, для которого они посчитаны.
    cache: Option<(u64, FilterKey, Vec<usize>)>,
}

impl Default for ConsoleView {
    fn default() -> Self {
        Self {
            session: None,
            followed: 0,
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
    fn filtered(&mut self, s: &Session) -> &[usize] {
        let key = (s.id, self.level, self.only_problems, self.only_own, self.search.to_lowercase());
        let fresh = matches!(&self.cache, Some((generation, k, _)) if *generation == s.generation && *k == key);
        if !fresh {
            let min = self.level.min();
            let query = key.4.clone();
            let rows = s
                .entries
                .iter()
                .enumerate()
                .filter(|(_, e)| e.level >= min || e.crash || (e.throwable.is_some() && min <= Level::Error))
                .filter(|(_, e)| !self.only_problems || (e.is_problem() && !e.noise))
                .filter(|(_, e)| !self.only_own || e.origin == mycelium_project::Origin::OwnMod)
                .filter(|(_, e)| query.is_empty() || e.search_text().to_lowercase().contains(&query))
                .map(|(i, _)| i)
                .collect();
            self.cache = Some((s.generation, key, rows));
        }
        &self.cache.as_ref().expect("посчитано выше").2
    }

    /// Какой запуск показывать: новый запуск перехватывает консоль, дальше — как выбрали.
    fn pick<'a>(&mut self, console: &'a Console) -> Option<&'a Session> {
        if console.latest != self.followed && console.session(console.latest).is_some() {
            self.followed = console.latest;
            self.session = Some(console.latest);
            self.selected = None;
            self.autoscroll = true;
        }
        let found = self.session.and_then(|id| console.session(id));
        let found = found.or_else(|| console.sessions.last());
        self.session = found.map(|s| s.id);
        found
    }
}

fn fmt_time(s: f64) -> String {
    let s = s.max(0.0) as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else if s >= 60 {
        format!("{}:{:02}", s / 60, s % 60)
    } else {
        t!("run-seconds", count = s)
    }
}

/// Панель запуска: выбор конфигурации, «Запустить» (или «Перезапустить», если уже идёт),
/// «Остановить» и правка конфигураций.
#[allow(clippy::too_many_arguments)]
pub fn run_bar(ui: &mut Ui, pal: &Palette, console: &Console, configs: &[RunConfig], selected: &mut Option<String>, cmds: &mut Vec<Cmd>) {
    let current = configs.iter().find(|c| Some(&c.id) == selected.as_ref()).or(configs.first());
    let label = current.map(|c| format!("{}  {}", c.kind.icon(), c.label())).unwrap_or_else(|| t!("run-no-configs"));
    egui::ComboBox::from_id_salt("mycelium-run-config").selected_text(label).width(190.0).show_ui(ui, |ui| {
        for c in configs {
            let running = console.is_running_config(&c.id);
            let text = format!("{}  {}{}", c.kind.icon(), c.label(), if running { "  ●" } else { "" });
            if ui.selectable_label(Some(&c.id) == selected.as_ref(), text).on_hover_text(c.command_line()).clicked() {
                *selected = Some(c.id.clone());
            }
        }
        ui.separator();
        if ui.button(format!("✎  {}", t!("run-edit-configs"))).clicked() {
            cmds.push(Cmd::EditRunConfigs);
        }
    });
    let running = current.is_some_and(|c| console.is_running_config(&c.id));
    let (glyph, color, hint) = if running { ("⟲", pal.accent, t!("run-restart")) } else { ("▶", pal.success, format!("{} (Shift+F10)", t!("run-start"))) };
    if ui.add_enabled(current.is_some(), egui::Button::new(RichText::new(glyph).color(color))).on_hover_text(hint).clicked() {
        cmds.push(Cmd::Run);
    }
    if running && ui.add(egui::Button::new(RichText::new("■").color(pal.danger))).on_hover_text(format!("{} (Ctrl+F2)", t!("run-stop"))).clicked() {
        cmds.push(Cmd::Stop);
    }
    if console.running().count() > 1 && ui.button(RichText::new("■■").color(pal.danger)).on_hover_text(t!("run-stop-all")).clicked() {
        cmds.push(Cmd::StopAll);
    }
}

fn status_text(pal: &Palette, s: &Session, now: f64) -> (String, egui::Color32) {
    match &s.status {
        Status::Running { since } => (t!("run-status-running", name = s.config.label(), time = fmt_time(now - since)), pal.accent),
        Status::Finished { code: Some(0), seconds } => (t!("run-status-done", name = s.config.label(), time = fmt_time(*seconds)), pal.success),
        Status::Finished { code, seconds } => (
            t!("run-status-failed", name = s.config.label(), time = fmt_time(*seconds), code = code.map(|c| c.to_string()).unwrap_or_else(|| "?".into())),
            pal.danger,
        ),
        Status::Stopped { seconds } => (t!("run-status-stopped", name = s.config.label(), time = fmt_time(*seconds)), pal.text_muted),
        Status::Failed(err) => (t!("run-status-error", error = err.clone()), pal.danger),
    }
}

/// Вкладки запусков: точка состояния, название, время; у законченных — крестик.
fn session_tabs(ui: &mut Ui, pal: &Palette, console: &Console, shown: Option<u64>, now: f64, view: &mut ConsoleView, cmds: &mut Vec<Cmd>) {
    ui.horizontal_wrapped(|ui| {
        ui.add_space(6.0);
        ui.spacing_mut().item_spacing.x = 4.0;
        for s in &console.sessions {
            let active = shown == Some(s.id);
            let (dot, time) = match &s.status {
                Status::Running { since } => (pal.success, fmt_time(now - since)),
                Status::Finished { code: Some(0), .. } => (pal.text_faint, String::new()),
                Status::Stopped { .. } => (pal.text_faint, String::new()),
                _ => (pal.danger, String::new()),
            };
            let label = format!("{}  {}", s.config.kind.icon(), s.config.label());
            let font = FontId::proportional(12.5);
            let g = ui.painter().layout_no_wrap(label, font.clone(), pal.text);
            let tg = ui.painter().layout_no_wrap(time.clone(), FontId::proportional(11.5), pal.text_faint);
            let close_w = if s.is_running() { 0.0 } else { 16.0 };
            let w = 22.0 + g.size().x + if time.is_empty() { 0.0 } else { tg.size().x + 8.0 } + close_w + 10.0;
            let (rect, resp) = ui.allocate_exact_size(vec2(w, 24.0), Sense::click());
            let fill = if active { pal.accent_soft } else if resp.hovered() { pal.bg_hover } else { pal.bg_header.gamma_multiply(0.6) };
            ui.painter().rect(rect, CornerRadius::same(7), fill, Stroke::new(1.0, if active { pal.accent.gamma_multiply(0.5) } else { pal.border }), egui::StrokeKind::Inside);
            ui.painter().circle_filled(rect.left_center() + vec2(11.0, 0.0), 3.5, dot);
            let text_pos = rect.left_center() + vec2(20.0, -g.size().y / 2.0);
            let gw = g.size().x;
            ui.painter().galley(text_pos, g, if active { pal.heading } else { pal.text });
            if !time.is_empty() {
                ui.painter().galley(rect.left_center() + vec2(28.0 + gw, -tg.size().y / 2.0), tg, pal.text_faint);
            }
            let mut closed = false;
            if !s.is_running() {
                let x = close_rect(rect);
                let hover = ui.input(|i| i.pointer.hover_pos()).is_some_and(|p| x.contains(p));
                ui.painter().text(x.center(), Align2::CENTER_CENTER, "×", FontId::proportional(13.0), if hover { pal.heading } else { pal.text_faint });
                if resp.clicked() && hover {
                    cmds.push(Cmd::CloseSession(s.id));
                    closed = true;
                }
            }
            let resp = resp.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(s.config.command_line());
            crate::theme::context_menu(&resp, |ui| {
                if s.is_running() && ui.button(t!("run-stop")).clicked() {
                    cmds.push(Cmd::StopSession(s.id));
                }
                if ui.button(t!("run-restart")).clicked() {
                    cmds.push(Cmd::RestartSession(s.id));
                }
                if !s.is_running() && ui.button(t!("run-close-tab")).clicked() {
                    cmds.push(Cmd::CloseSession(s.id));
                }
            });
            if resp.middle_clicked() && !s.is_running() {
                cmds.push(Cmd::CloseSession(s.id));
            } else if resp.clicked() && !closed {
                view.session = Some(s.id);
                view.selected = None;
            }
        }
    });
}

/// Место крестика на вкладке запуска.
fn close_rect(tab: egui::Rect) -> egui::Rect {
    egui::Rect::from_center_size(tab.right_center() - vec2(11.0, 0.0), vec2(14.0, 14.0))
}

#[allow(clippy::too_many_arguments)]
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
    let session = view.pick(console);
    let shown = session.map(|s| s.id);

    // Панель: запуск и состояние слева, фильтры справа.
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        run_bar(ui, pal, console, configs, selected_config, cmds);
        if let Some(s) = session {
            let (text, color) = status_text(pal, s, now);
            ui.label(RichText::new(text).size(12.5).color(color));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(6.0);
            if let Some(id) = shown {
                if ui.button("🗑").on_hover_text(t!("console-clear")).clicked() {
                    cmds.push(Cmd::ClearSession(id));
                    view.selected = None;
                }
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
    if console.sessions.len() > 1 {
        ui.add_space(2.0);
        session_tabs(ui, pal, console, shown, now, view, cmds);
    }
    ui.add_space(3.0);

    let Some(session) = session else {
        super::empty_state(ui, pal, "🖥", &t!("console-empty-title"), &t!("console-empty-hint"));
        return;
    };
    let rows: Vec<usize> = view.filtered(session).to_vec();
    if rows.is_empty() {
        let (title, hint) = if session.entries.is_empty() {
            (t!("console-waiting-title"), t!("console-waiting-hint"))
        } else {
            (t!("console-filtered-title"), t!("console-filtered-hint"))
        };
        super::empty_state(ui, pal, "🖥", &title, &hint);
        return;
    }

    let selected_entry = view.selected.and_then(|seq| session.entries.iter().find(|e| e.seq == seq));
    let total_h = ui.available_height();
    let list_h = if selected_entry.is_some() { (total_h * 0.55).max(120.0) } else { total_h };

    // Список записей: одинаковая высота строк — рисуются только видимые.
    ui.allocate_ui(vec2(ui.available_width(), list_h), |ui| {
        let mut area = egui::ScrollArea::vertical().id_salt(("mycelium-console-list", session.id)).auto_shrink(false);
        if view.autoscroll {
            area = area.stick_to_bottom(true);
        }
        area.show_rows(ui, ROW_H, rows.len(), |ui, range| {
            for &i in &rows[range] {
                let e = &session.entries[i];
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
