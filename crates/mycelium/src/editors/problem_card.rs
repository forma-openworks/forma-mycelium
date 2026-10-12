//! Карточка проблемы — в окне исключений и под логом консоли.
//!
//! Сверху вниз: заголовок (подстановки — токены) и действия справа; тихая строка
//! «когда и где» (время, сторона запуска, поток); плашки «что и чьё» (уровень, система,
//! владелец, через кого всплыло); сообщение в тёмной панели; объяснение простыми словами;
//! свёрнутый стек вызовов. В узкой области действия уходят под заголовок, а «Копировать»
//! становится значком.

use std::collections::HashSet;
use std::path::PathBuf;

use eframe::egui::{self, Align, Color32, CornerRadius, CursorIcon, FontId, Layout, Margin, RichText, Sense, Stroke, Ui, vec2};
use forma_i18n::t;
use forma_ui::theme::Palette;
use mycelium_log::{Entry, Known, Role, RunSide, System, ThreadKind, Throwable};
use mycelium_project::{Libraries, Origin};

use super::problem_ui::{self, entry_scope, level_color, level_tag, origin_label, origin_tag, system_label};
use super::token_text::{self, TokenCx};
use mycelium_log::tokens::tokens;
use crate::app::Cmd;

/// Сколько раз и когда повторялась проблема (окно исключений). В консоли — одна запись.
pub struct Repeats {
    pub count: u32,
    pub first: Option<String>,
    pub last: Option<String>,
    /// «новая в этом запуске», «не было в последнем запуске».
    pub note: Option<String>,
}

/// Главное действие карточки — зависит от владельца проблемы.
enum Primary {
    /// Чужой мод с трекером задач: скопировать отчёт и открыть трекер.
    Report { url: String, text: String },
    /// Свой мод: открыть место в коде (файл или класс, при желании на строке).
    Open { path: Option<String>, class: Option<String>, file: Option<String>, line: Option<u32> },
    /// Свой мод, проблема ресурса: открыть файл, о котором речь (модель, blockstate, данные).
    Resource(PathBuf),
}

impl Primary {
    fn cmd(&self) -> Cmd {
        match self {
            Primary::Report { url, text } => Cmd::ReportIssue { url: url.clone(), text: text.clone() },
            Primary::Open { path: Some(path), line, .. } => Cmd::OpenFile { path: path.into(), line: *line },
            Primary::Open { class, file, line, .. } => Cmd::OpenSource { class: class.clone().unwrap_or_default(), file: file.clone(), line: *line },
            Primary::Resource(path) => Cmd::OpenFile { path: path.clone(), line: None },
        }
    }
}

/// Карточка записи. `expanded` — какие свёрнутые группы кадров стека раскрыты.
pub fn card(ui: &mut Ui, pal: &Palette, e: &Entry, repeats: Option<&Repeats>, libs: Option<&Libraries>, expanded: &mut HashSet<(u64, usize, usize)>, cmds: &mut Vec<Cmd>) {
    let cx = TokenCx::new(pal, libs, &e.fields);
    let scope = entry_scope(e);
    let explained = e.known.is_some() || e.noise;
    let r = 10u8;
    // Внутренние панели отступают на 1 px — рамка карточки остаётся видна.
    egui::Frame::new().fill(pal.bg_panel).stroke(Stroke::new(1.0, pal.border)).corner_radius(r).inner_margin(1).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 0.0;
        egui::Frame::new().inner_margin(Margin { left: 16, right: 14, top: 12, bottom: 12 }).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            header(ui, pal, e, &cx, scope, cmds);
            ui.add_space(4.0);
            meta_row(ui, pal, e, repeats);
            ui.add_space(6.0);
            chips_row(ui, pal, e);
        });

        // Сообщение — в тёмной панели: здесь говорит игра, а не IDE.
        let last = !explained && e.throwable.is_none();
        let bottom = if last { r - 1 } else { 0 };
        egui::Frame::new().fill(pal.bg_deep).corner_radius(CornerRadius { nw: 0, ne: 0, sw: bottom, se: bottom }).inner_margin(Margin::symmetric(16, 12)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 3.0;
            message(ui, pal, e, &cx, scope, cmds);
        });

        if explained {
            egui::Frame::new().inner_margin(Margin::symmetric(16, 12)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                match e.known {
                    Some(k) => explanation(ui, pal, "hint", pal.accent, &problem_ui::known_title(k), &problem_ui::known_explain(k)),
                    None => explanation(ui, pal, "noise", pal.text_faint, &t!("noise-title"), &t!("noise-explain")),
                }
            });
        }
        if let Some(t) = &e.throwable {
            if explained {
                separator(ui, pal);
            }
            stack(ui, &cx, scope, e, t, expanded, cmds);
        }
    });
}

/// Заголовок и действия. Не хватает места рядом — действия переходят под заголовок.
fn header(ui: &mut Ui, pal: &Palette, e: &Entry, cx: &TokenCx, scope: egui::Id, cmds: &mut Vec<Cmd>) {
    let primary = primary_action(ui, e, cx, scope);
    let avail = ui.available_width();
    let compact = avail < 420.0;
    let copied_key = scope.with("copied");
    let now = ui.input(|i| i.time);
    let copied = ui.data(|d| d.get_temp::<f64>(copied_key)).is_some_and(|at| now - at < 1.5);
    let copy_label = if copied { t!("problem-copied") } else { t!("problem-copy") };
    let primary_label = primary.as_ref().map(|p| match p {
        Primary::Report { .. } => t!("problem-report"),
        Primary::Open { .. } => t!("problem-open-code"),
        Primary::Resource(path) => t!("problem-open-resource", name = file_name(path)),
    });
    let copy_w = action_width(ui, (!compact).then_some(copy_label.as_str()));
    let primary_w = primary_label.as_deref().map_or(0.0, |l| action_width(ui, Some(l)) + 6.0);
    let actions_w = copy_w + primary_w;

    // Действия только сообщают о нажатии: команды отправляются после раскладки.
    let mut pressed = false;
    let mut actions = |ui: &mut Ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        if let (Some(p), Some(label)) = (&primary, &primary_label) {
            let (icon, hint) = match p {
                Primary::Report { .. } => ("report", t!("problem-report-hint")),
                Primary::Open { .. } => ("file-code", t!("problem-open-code-hint")),
                Primary::Resource(path) => (super::project::file_icon(&file_name(path), false, false).0, t!("problem-open-resource-hint")),
            };
            if action_button(ui, pal, icon, Some(label), true).on_hover_text(hint).clicked() {
                pressed = true;
            }
        }
        let copy = action_button(ui, pal, if copied { "check" } else { "copy" }, (!compact).then_some(copy_label.as_str()), false);
        if copy.on_hover_text(t!("problem-copy-hint")).clicked() {
            ui.ctx().copy_text(entry_text(e));
            ui.data_mut(|d| d.insert_temp(copied_key, now));
        }
        if copied {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(300));
        }
    };

    let font = FontId::new(17.0, forma_ui::fonts::semibold());
    let mut title = |ui: &mut Ui| match e.known.and_then(|k| token_text::headline_pieces(k, cx, &font, pal.heading)) {
        Some(pieces) => {
            token_text::show(ui, scope, pieces, cx, cmds);
        }
        None => {
            ui.add(egui::Label::new(RichText::new(problem_ui::entry_title(e)).font(font.clone()).color(pal.heading)).wrap());
        }
    };
    if avail - actions_w - 16.0 >= 220.0 {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let title_w = avail - actions_w - 16.0;
            let used = ui
                .allocate_ui_with_layout(vec2(title_w, 0.0), Layout::top_down(Align::Min), |ui| {
                    ui.set_max_width(title_w);
                    title(ui);
                })
                .response
                .rect
                .width();
            // Действия прижаты к правому краю, какой бы короткий ни был заголовок.
            ui.add_space(title_w - used + 16.0);
            ui.horizontal(|ui| actions(ui));
        });
    } else {
        title(ui);
        ui.add_space(4.0);
        ui.horizontal(|ui| actions(ui));
    }
    if pressed && let Some(p) = &primary {
        cmds.push(p.cmd());
    }
}

/// Главное действие: отчёт автору чужого мода, место в своём коде или файл своего ресурса.
fn primary_action(ui: &Ui, e: &Entry, cx: &TokenCx, scope: egui::Id) -> Option<Primary> {
    match &e.origin {
        Origin::OtherMod(Some(id)) => {
            let run = cx.run.as_deref()?;
            let m = run.get(id)?;
            let url = m.issues.clone()?;
            Some(Primary::Report { text: report_text(e, run, m), url })
        }
        Origin::OwnMod => {
            if let Some(loc) = &e.location {
                return Some(Primary::Open { path: Some(loc.file.clone()), class: None, file: None, line: Some(loc.line) });
            }
            if let Some(frame) = e.throwable.iter().flat_map(|t| t.chain()).flat_map(|c| c.frames.iter()).find(|f| f.origin == Origin::OwnMod) {
                return Some(Primary::Open { path: None, class: Some(frame.class.clone()), file: frame.file.clone(), line: frame.line });
            }
            own_resource_file(ui, e, cx, scope).map(Primary::Resource)
        }
        _ => None,
    }
}

/// Первый существующий файл своего ресурса, названный в записи: модель, blockstate,
/// файл данных. Диск проверяется не чаще раза в две секунды — файл могли создать.
fn own_resource_file(ui: &Ui, e: &Entry, cx: &TokenCx, scope: egui::Id) -> Option<PathBuf> {
    let key = scope.with("resource-file");
    let now = ui.input(|i| i.time);
    if let Some((at, found)) = ui.data(|d| d.get_temp::<(f64, Option<PathBuf>)>(key))
        && now - at < 2.0
    {
        return found;
    }
    let dirs = crate::mods_view::resource_dirs();
    let texts = e.preceding.iter().chain(std::iter::once(&e.message)).chain(e.details.iter());
    let found = texts
        .flat_map(|text| tokens(text, &e.fields).into_iter().map(move |t| (t.role, text[t.range].to_owned())))
        .flat_map(|(role, value)| match role {
            // Только свои пространства имён: ресурсов Minecraft в проекте нет.
            Role::Resource if cx.rules.namespace(value.split(':').next().unwrap_or("")) == Origin::OwnMod => resource_candidates(&value),
            Role::File if !std::path::Path::new(&value).is_absolute() => vec![value.replace('\\', "/")],
            _ => Vec::new(),
        })
        .find_map(|rel| dirs.iter().map(|d| d.join(&rel)).find(|p| p.is_file()));
    ui.data_mut(|d| d.insert_temp(key, (now, found.clone())));
    found
}

/// Файлы, которые может означать id ресурса, — от вероятного к менее вероятному (пути от
/// папки ресурсов). `testmod:ruby#inventory` — модель предмета, `testmod:item/ruby` — модель
/// или текстура, `testmod:recipe/ruby.json` — путь в `data/` или `assets/` как есть.
fn resource_candidates(id: &str) -> Vec<String> {
    let id = id.split('#').next().unwrap_or(id);
    let Some((ns, path)) = id.split_once(':') else { return Vec::new() };
    if path.is_empty() {
        return Vec::new();
    }
    if path.rsplit('/').next().is_some_and(|f| f.contains('.')) {
        return vec![format!("assets/{ns}/{path}"), format!("data/{ns}/{path}")];
    }
    let mut out = vec![format!("assets/{ns}/models/{path}.json")];
    if !path.contains('/') {
        for dir in ["models/item", "models/block", "blockstates", "items"] {
            out.push(format!("assets/{ns}/{dir}/{path}.json"));
        }
    }
    out.push(format!("assets/{ns}/textures/{path}.png"));
    out
}

fn file_name(path: &std::path::Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Строка «когда и где»: повторы и время, сторона запуска, поток. Тихая — без рамок.
fn meta_row(ui: &mut Ui, pal: &Palette, e: &Entry, repeats: Option<&Repeats>) {
    let size = 12.0;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(14.0, 4.0);
        let short = |t: &str| super::console::short_time(t);
        let when = match repeats {
            Some(r) => {
                let range = match (&r.first, &r.last) {
                    (Some(a), Some(b)) if short(a) != short(b) => Some(format!("{} – {}", short(a), short(b))),
                    (Some(a), _) => Some(short(a)),
                    _ => None,
                };
                // Счётчик — только когда повторов больше одного: «1 раз» — шум.
                match (r.count > 1, range) {
                    (true, Some(range)) => Some(format!("×{} · {range}", r.count)),
                    (true, None) => Some(format!("×{}", r.count)),
                    (false, range) => range,
                }
            }
            None => e.time.as_deref().map(short),
        };
        if let Some(when) = when {
            meta_item(ui, Some("time"), &when, size, pal.text_faint);
        }
        if let Some(note) = repeats.and_then(|r| r.note.as_deref()) {
            meta_item(ui, None, note, size, pal.accent);
        }
        if let Some(side) = e.run_side {
            let (icon, label, hint) = match side {
                RunSide::Client => ("run-client", t!("problem-side-client"), t!("problem-side-client-hint")),
                RunSide::Server => ("run-server", t!("problem-side-server"), t!("problem-side-server-hint")),
            };
            meta_item(ui, Some(icon), &label, size, pal.text_faint).on_hover_text(hint);
        }
        if let Some(thread) = &e.thread {
            // Длинное имя потока не раздвигает узкую область, а обрезается.
            let text = format!("{} {thread}", t!("problem-thread"));
            let resp = meta_item(ui, Some("thread"), &text, size, pal.text_faint);
            let hint = ThreadKind::of(thread).map(|k| forma_i18n::tr(&format!("thread-{}-hint", k.key())));
            match hint {
                Some(h) => {
                    resp.on_hover_text(format!("{text}\n{h}"));
                }
                None => {
                    resp.on_hover_text(text);
                }
            }
        }
    });
}

/// Пункт строки «когда и где»: значок и текст в одну строку, выровненные по центру —
/// у всех пунктов одинаково, поэтому строка не скачет. Не помещается — обрезается «…»
/// (полный текст подсказки задаёт вызывающий).
fn meta_item(ui: &mut Ui, icon: Option<&str>, text: &str, size: f32, color: Color32) -> egui::Response {
    let icon_w = if icon.is_some() { size + 5.0 } else { 0.0 };
    let max = (ui.available_width() - icon_w).clamp(60.0, 280.0);
    let galley = forma_ui::widgets::elide(ui, text, FontId::proportional(size), color, max);
    let h = galley.size().y.max(size + 2.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(icon_w + galley.size().x, h), Sense::hover());
    if let Some(icon) = icon {
        forma_ui::icons::paint(ui.painter(), egui::Rect::from_center_size(egui::pos2(rect.left() + size / 2.0, rect.center().y), vec2(size, size)), icon, color);
    }
    ui.painter().galley(egui::pos2(rect.left() + icon_w, rect.center().y - galley.size().y / 2.0), galley, color);
    resp
}

/// Плашки «что и чьё»: уровень, система, владелец и через кого проблема всплыла.
fn chips_row(ui: &mut Ui, pal: &Palette, e: &Entry) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        level_chip(ui, pal, e);
        outline_chip(ui, pal, Some(system_icon(e.system)), &system_label(e.system), pal.text_muted);
        origin_tag(ui, pal, &e.origin).on_hover_text(t!("problem-owner-hint"));
        if let Some(via) = &e.via {
            let text = format!("{} {}", t!("problem-via"), origin_label(via));
            let resp = outline_chip(ui, pal, Some("via"), &text, pal.text_muted);
            match &e.logger {
                Some(logger) => resp.on_hover_text(format!("{}\n{logger}", t!("problem-via-hint"))),
                None => resp.on_hover_text(t!("problem-via-hint")),
            };
        }
    });
}

fn level_chip(ui: &mut Ui, pal: &Palette, e: &Entry) {
    let (text, color) = if e.crash { ("CRASH", pal.danger) } else { (level_tag(e.level), level_color(pal, e.level)) };
    let font = FontId::new(11.5, forma_ui::fonts::semibold());
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, color);
    let (rect, _) = ui.allocate_exact_size(galley.size() + vec2(24.0, 8.0), Sense::hover());
    ui.painter().rect(rect, CornerRadius::same(11), color.gamma_multiply(0.14), Stroke::new(1.0, color.gamma_multiply(0.35)), egui::StrokeKind::Inside);
    ui.painter().circle_filled(rect.left_center() + vec2(10.0, 0.0), 3.0, color);
    ui.painter().galley(rect.left_top() + vec2(17.0, 4.0), galley, color);
}

/// Плашка с рамкой: значок и подпись. В узком ряду подпись обрезается.
fn outline_chip(ui: &mut Ui, pal: &Palette, icon: Option<&str>, text: &str, color: Color32) -> egui::Response {
    let font = FontId::proportional(12.0);
    let lead = if icon.is_some() { 26.0 } else { 10.0 };
    let max = (ui.max_rect().width() - lead - 10.0).max(24.0);
    let galley = forma_ui::widgets::elide(ui, text, font, color, max);
    let elided = galley.elided;
    let (rect, resp) = ui.allocate_exact_size(galley.size() + vec2(lead + 10.0, 8.0), Sense::hover());
    ui.painter().rect_stroke(rect, CornerRadius::same(11), Stroke::new(1.0, pal.border_strong), egui::StrokeKind::Inside);
    if let Some(icon) = icon {
        forma_ui::icons::paint(ui.painter(), egui::Rect::from_center_size(rect.left_center() + vec2(15.0, 0.0), vec2(13.0, 13.0)), icon, color);
    }
    ui.painter().galley(rect.left_top() + vec2(lead, 4.0), galley, color);
    if elided { resp.on_hover_text(text) } else { resp }
}

fn system_icon(system: System) -> &'static str {
    match system {
        System::ModLoading => "system-mod-loading",
        System::Registries => "system-registries",
        System::Resources => "system-resources",
        System::Data => "system-data",
        System::Mixin => "system-mixin",
        System::Network => "system-network",
        System::Rendering => "system-rendering",
        System::Sound => "system-sound",
        System::World => "system-world",
        System::Config => "system-config",
        System::Build => "system-build",
        System::Game => "system-game",
    }
}

/// Сообщение, продолжение и место в файле (у ошибок компилятора).
fn message(ui: &mut Ui, pal: &Palette, e: &Entry, cx: &TokenCx, scope: egui::Id, cmds: &mut Vec<Cmd>) {
    let mono = FontId::monospace(12.5);
    let bad_ids = matches!(e.known, Some(Known::InvalidResourceLocation | Known::InvalidResourcePath));
    // Строки перед сообщением (`Error loading class` перед `@Mixin target`) — приглушённо.
    for p in &e.preceding {
        token_text::show(ui, scope, token_text::message_pieces(p, cx, &mono, pal.text_muted, bad_ids), cx, cmds);
    }
    token_text::show(ui, scope, token_text::message_pieces(&e.message, cx, &mono, pal.heading, bad_ids), cx, cmds);
    for d in &e.details {
        token_text::show(ui, scope, token_text::message_pieces(d, cx, &mono, pal.text_muted, bad_ids), cx, cmds);
    }
    if let Some(loc) = &e.location {
        let text = format!("{}:{}", loc.file, loc.line);
        let jump = ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
            forma_ui::icons::paint(ui.painter(), r, "jump", pal.accent);
            ui.add(egui::Label::new(RichText::new(text).font(mono.clone()).color(pal.accent)).sense(Sense::click()))
        });
        if jump.inner.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            cmds.push(Cmd::OpenFile { path: loc.file.clone().into(), line: Some(loc.line) });
        }
    }
}

/// Объяснение простыми словами: значок, заголовок, текст.
fn explanation(ui: &mut Ui, pal: &Palette, icon: &str, tint: Color32, title: &str, text: &str) {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let (r, _) = ui.allocate_exact_size(vec2(16.0, 18.0), Sense::hover());
        forma_ui::icons::paint(ui.painter(), egui::Rect::from_center_size(r.center(), vec2(16.0, 16.0)), icon, tint);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 3.0;
            ui.add(egui::Label::new(RichText::new(title).size(13.5).color(pal.heading).family(forma_ui::fonts::semibold())).wrap());
            ui.add(egui::Label::new(RichText::new(text).size(13.0).color(pal.text)).wrap());
        });
    });
}

fn separator(ui: &mut Ui, pal: &Palette) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(r, CornerRadius::ZERO, pal.border);
}

/// Стек вызовов: строка «Показать стек · 19 кадров · 2 твоих» и сам стек под ней.
/// Открыт сразу, если в нём есть кадры своего мода или игра упала.
fn stack(ui: &mut Ui, cx: &TokenCx, scope: egui::Id, e: &Entry, t: &Throwable, expanded: &mut HashSet<(u64, usize, usize)>, cmds: &mut Vec<Cmd>) {
    let pal = cx.pal;
    let frames = t.chain().map(|c| c.frames.len()).sum::<usize>();
    let own = t.chain().flat_map(|c| c.frames.iter()).filter(|f| f.origin == Origin::OwnMod).count();
    let key = scope.with("stack-open");
    let open = ui.data(|d| d.get_temp::<bool>(key)).unwrap_or(own > 0 || e.crash);
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius { nw: 0, ne: 0, sw: if open { 0 } else { 9 }, se: if open { 0 } else { 9 } }, pal.bg_hover.gamma_multiply(0.6));
    }
    let painter = ui.painter_at(rect);
    let mut x = rect.left() + 16.0;
    forma_ui::icons::paint(&painter, egui::Rect::from_center_size(egui::pos2(x + 8.0, rect.center().y), vec2(15.0, 15.0)), "stack", pal.text_muted);
    x += 24.0;
    let label = painter.layout_no_wrap(if open { t!("problem-stack-hide") } else { t!("problem-stack-show") }, FontId::proportional(13.0), pal.text);
    let w = label.size().x;
    painter.galley(egui::pos2(x, rect.center().y - label.size().y / 2.0), label, pal.text);
    x += w + 10.0;
    let mut count = t!("frames-hidden", count = frames);
    if own > 0 {
        count = format!("{count} · {}", t!("problem-stack-yours", count = own));
    }
    let badge = painter.layout_no_wrap(count, FontId::proportional(11.5), pal.text_muted);
    let badge_rect = egui::Rect::from_min_size(egui::pos2(x, rect.center().y - badge.size().y / 2.0 - 2.0), badge.size() + vec2(12.0, 4.0));
    if badge_rect.right() < rect.right() - 30.0 {
        painter.rect_filled(badge_rect, CornerRadius::same(5), pal.bg_hover);
        painter.galley(badge_rect.min + vec2(6.0, 2.0), badge, pal.text_muted);
    }
    forma_ui::icons::paint(
        &painter,
        egui::Rect::from_center_size(egui::pos2(rect.right() - 20.0, rect.center().y), vec2(14.0, 14.0)),
        if open { "chevron-up" } else { "chevron-down" },
        pal.text_muted,
    );
    if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
        ui.data_mut(|d| d.insert_temp(key, !open));
    }
    if open {
        egui::Frame::new().inner_margin(Margin { left: 16, right: 14, top: 0, bottom: 12 }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 2.0;
            problem_ui::throwable(ui, cx, scope, e.seq, t, expanded, cmds);
        });
    }
}

fn action_width(ui: &Ui, label: Option<&str>) -> f32 {
    let text = label.map_or(0.0, |l| ui.painter().layout_no_wrap(l.to_owned(), FontId::proportional(13.0), Color32::WHITE).size().x + 6.0);
    10.0 + 15.0 + text + 10.0
}

/// Кнопка действия: значок и подпись (без подписи — один значок). Главное действие — в рамке акцента.
fn action_button(ui: &mut Ui, pal: &Palette, icon: &str, label: Option<&str>, accent: bool) -> egui::Response {
    let w = action_width(ui, label);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
    let hovered = resp.hovered();
    let color = if accent { pal.accent } else { pal.text };
    if accent {
        let fill = pal.accent.gamma_multiply(if hovered { 0.18 } else { 0.08 });
        ui.painter().rect(rect, CornerRadius::same(7), fill, Stroke::new(1.0, pal.accent.gamma_multiply(0.7)), egui::StrokeKind::Inside);
    } else if hovered {
        ui.painter().rect_filled(rect, CornerRadius::same(7), pal.bg_hover);
    }
    forma_ui::icons::paint(ui.painter(), egui::Rect::from_center_size(rect.left_center() + vec2(17.5, 0.0), vec2(15.0, 15.0)), icon, color);
    if let Some(label) = label {
        let galley = ui.painter().layout_no_wrap(label.to_owned(), FontId::proportional(13.0), color);
        ui.painter().galley(egui::pos2(rect.left() + 31.0, rect.center().y - galley.size().y / 2.0), galley, color);
    }
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// Записи как в логе: строка с временем, потоком, уровнем и логгером, продолжение и стек.
pub fn entry_text(e: &Entry) -> String {
    let mut head = String::new();
    if let Some(t) = &e.time {
        head.push_str(&format!("[{t}] "));
    }
    let level = format!("{:?}", e.level).to_uppercase();
    match &e.thread {
        Some(th) => head.push_str(&format!("[{th}/{level}] ")),
        None => head.push_str(&format!("[{level}] ")),
    }
    if let Some(l) = &e.logger {
        head.push_str(&format!("[{l}]: "));
    }
    let mut out = String::new();
    for p in &e.preceding {
        out.push_str(&format!("{head}{p}\n"));
    }
    out.push_str(&format!("{head}{}", e.message));
    for d in &e.details {
        out.push('\n');
        out.push_str(d);
    }
    if let Some(t) = &e.throwable {
        for (i, c) in t.chain().enumerate() {
            out.push('\n');
            if i > 0 {
                out.push_str("Caused by: ");
            }
            out.push_str(&c.type_name);
            if let Some(m) = &c.message {
                out.push_str(": ");
                out.push_str(m);
            }
            for f in &c.frames {
                let place = match (&f.file, f.line) {
                    (Some(file), Some(line)) => format!("{file}:{line}"),
                    (Some(file), None) => file.clone(),
                    _ => "Unknown Source".to_owned(),
                };
                out.push_str(&format!("\n\tat {}.{}({place})", f.class, f.method));
            }
            if c.omitted > 0 {
                out.push_str(&format!("\n\t... {} more", c.omitted));
            }
        }
    }
    out
}

/// Отчёт для автора мода (по-английски — его читают не только русскоязычные авторы):
/// версии мода, игры и загрузчика и строки лога как есть.
fn report_text(e: &Entry, run: &mycelium_hyphae::RunMods, m: &mycelium_hyphae::RunMod) -> String {
    let mut out = format!("**Mod:** {} `{}` {}\n", m.name, m.id, m.version);
    let mut env = Vec::new();
    if let Some(mc) = run.get("minecraft").filter(|x| !x.version.is_empty()) {
        env.push(format!("**Minecraft:** {}", mc.version));
    }
    for loader in ["neoforge", "fabricloader", "forge"] {
        if let Some(l) = run.get(loader).filter(|x| !x.version.is_empty()) {
            env.push(format!("**{}:** {}", l.name, l.version));
        }
    }
    if !env.is_empty() {
        out.push_str(&env.join(" · "));
        out.push('\n');
    }
    if let Some(mixin) = e.field(Role::Mixin) {
        out.push_str(&format!("**Mixin:** `{mixin}`\n"));
    }
    out.push_str("\n```\n");
    out.push_str(&entry_text(e));
    out.push_str("\n```\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use mycelium_log::{Frame, Level, Source};

    #[test]
    fn resource_ids_point_to_files() {
        assert_eq!(resource_candidates("testmod:ruby#inventory")[..2], ["assets/testmod/models/ruby.json".to_owned(), "assets/testmod/models/item/ruby.json".to_owned()]);
        assert!(resource_candidates("testmod:item/ruby").contains(&"assets/testmod/textures/item/ruby.png".to_owned()));
        assert_eq!(resource_candidates("testmod:recipe/ruby_block.json"), ["assets/testmod/recipe/ruby_block.json", "data/testmod/recipe/ruby_block.json"]);
        assert!(resource_candidates("testmod").is_empty());
    }

    #[test]
    fn copied_text_looks_like_the_log() {
        let mut e = Entry::new(1, Level::Warn, Source::Game, "Something broke");
        e.time = Some("11Oct2026 14:50:17.310".into());
        e.thread = Some("Render thread".into());
        e.logger = Some("net.minecraft.client.Minecraft".into());
        let mut t = Throwable::new("java.lang.IllegalStateException", Some("bad".into()));
        t.frames.push(Frame { module: None, class: "com.holo.testmod.Gems".into(), method: "describe".into(), file: Some("Gems.java".into()), line: Some(10), jar: None, origin: Origin::OwnMod });
        t.omitted = 3;
        e.throwable = Some(t);
        assert_eq!(
            entry_text(&e),
            "[11Oct2026 14:50:17.310] [Render thread/WARN] [net.minecraft.client.Minecraft]: Something broke\njava.lang.IllegalStateException: bad\n\tat com.holo.testmod.Gems.describe(Gems.java:10)\n\t... 3 more"
        );
    }
}
