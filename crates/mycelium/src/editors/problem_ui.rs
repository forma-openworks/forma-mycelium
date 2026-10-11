//! Общие элементы консоли и окна исключений: цвета происхождения, подписи систем и уровней,
//! объяснения известных проблем и стек-трейс, в котором чужие кадры свёрнуты.

use std::collections::HashSet;
use std::sync::LazyLock;

use eframe::egui::{self, Color32, CornerRadius, CursorIcon, FontId, RichText, Sense, Stroke, Ui, text::LayoutJob, text::TextFormat, vec2};
use regex::Regex;
use forma_i18n::t;
use mycelium_log::{Entry, Frame, Known, Level, System, Throwable};
use mycelium_project::{Libraries, Origin};

use crate::app::Cmd;
use forma_ui::theme::Palette;

/// Цвет происхождения — один и тот же во всей IDE.
pub fn origin_color(_pal: &Palette, origin: &Origin) -> Color32 {
    let c = crate::colors::current();
    match origin {
        Origin::OwnMod => c.own,
        Origin::Minecraft => c.minecraft,
        Origin::NeoForge => c.neoforge,
        Origin::Fabric => c.fabric,
        Origin::Forge => c.forge,
        Origin::OtherMod(_) => c.other_mod,
        Origin::Library => c.library,
        Origin::Java => c.java,
        Origin::Unknown => c.unknown,
    }
}

/// Подпись происхождения. У модов — «Название (id)», если мод есть среди модов запуска.
pub fn origin_label(origin: &Origin) -> String {
    if let Some(id) = crate::mods_view::mod_id(origin) {
        let label = crate::mods_view::mod_label(&id);
        if label != id {
            return label;
        }
    }
    match origin {
        Origin::OwnMod => t!("origin-own"),
        Origin::Minecraft => "Minecraft".to_owned(),
        Origin::NeoForge => "NeoForge".to_owned(),
        Origin::Fabric => "Fabric".to_owned(),
        Origin::Forge => "Forge".to_owned(),
        Origin::OtherMod(Some(id)) => t!("origin-mod-named", id = id.clone()),
        Origin::OtherMod(None) => t!("origin-mod"),
        Origin::Library => t!("origin-library"),
        Origin::Java => "Java".to_owned(),
        Origin::Unknown => t!("origin-unknown"),
    }
}

pub fn system_label(system: System) -> String {
    match system {
        System::ModLoading => t!("system-mod-loading"),
        System::Registries => t!("system-registries"),
        System::Resources => t!("system-resources"),
        System::Data => t!("system-data"),
        System::Mixin => t!("system-mixin"),
        System::Network => t!("system-network"),
        System::Rendering => t!("system-rendering"),
        System::Sound => t!("system-sound"),
        System::World => t!("system-world"),
        System::Config => t!("system-config"),
        System::Build => t!("system-build"),
        System::Game => t!("system-game"),
    }
}

pub fn level_color(_pal: &Palette, level: Level) -> Color32 {
    let c = crate::colors::current();
    match level {
        Level::Fatal | Level::Error => c.level_error,
        Level::Warn => c.level_warn,
        Level::Info => c.level_info,
        Level::Debug | Level::Trace => c.level_debug,
    }
}

pub fn level_tag(level: Level) -> &'static str {
    match level {
        Level::Trace => "TRACE",
        Level::Debug => "DEBUG",
        Level::Info => "INFO",
        Level::Warn => "WARN",
        Level::Error => "ERROR",
        Level::Fatal => "FATAL",
    }
}

pub fn known_title(k: Known) -> String {
    match k {
        Known::MissingTexture => t!("known-missing-texture-title"),
        Known::MissingModel => t!("known-missing-model-title"),
        Known::BlockstateError => t!("known-blockstate-title"),
        Known::InvalidResourceLocation => t!("known-invalid-id-title"),
        Known::ClientClassOnServer => t!("known-client-on-server-title"),
        Known::MixinFailure => t!("known-mixin-title"),
        Known::RegistryNotBound => t!("known-registry-unbound-title"),
        Known::ClassMissing => t!("known-class-missing-title"),
        Known::NullPointer => t!("known-null-pointer-title"),
        Known::DataParseError => t!("known-data-parse-title"),
        Known::MissingDependency => t!("known-missing-dependency-title"),
        Known::DuplicateMod => t!("known-duplicate-mod-title"),
        Known::ModConstruction => t!("known-mod-construction-title"),
        Known::OutOfMemory => t!("known-out-of-memory-title"),
        Known::ConcurrentModification => t!("known-concurrent-modification-title"),
        Known::CompileError => t!("known-compile-error-title"),
        Known::BuildFailed => t!("known-build-failed-title"),
        Known::ObjMaterial => t!("known-obj-material-title"),
        Known::ObjFile => t!("known-obj-file-title"),
        Known::InvalidResourcePath => t!("known-invalid-path-title"),
    }
}

pub fn known_explain(k: Known) -> String {
    match k {
        Known::MissingTexture => t!("known-missing-texture-explain"),
        Known::MissingModel => t!("known-missing-model-explain"),
        Known::BlockstateError => t!("known-blockstate-explain"),
        Known::InvalidResourceLocation => t!("known-invalid-id-explain"),
        Known::ClientClassOnServer => t!("known-client-on-server-explain"),
        Known::MixinFailure => t!("known-mixin-explain"),
        Known::RegistryNotBound => t!("known-registry-unbound-explain"),
        Known::ClassMissing => t!("known-class-missing-explain"),
        Known::NullPointer => t!("known-null-pointer-explain"),
        Known::DataParseError => t!("known-data-parse-explain"),
        Known::MissingDependency => t!("known-missing-dependency-explain"),
        Known::DuplicateMod => t!("known-duplicate-mod-explain"),
        Known::ModConstruction => t!("known-mod-construction-explain"),
        Known::OutOfMemory => t!("known-out-of-memory-explain"),
        Known::ConcurrentModification => t!("known-concurrent-modification-explain"),
        Known::CompileError => t!("known-compile-error-explain"),
        Known::BuildFailed => t!("known-build-failed-explain"),
        Known::ObjMaterial => t!("known-obj-material-explain"),
        Known::ObjFile => t!("known-obj-file-explain"),
        Known::InvalidResourcePath => t!("known-invalid-path-explain"),
    }
}

/// Короткое название проблемы: известная, тип исключения или текст сообщения.
pub fn entry_title(e: &Entry) -> String {
    if let Some(k) = e.known {
        return known_title(k);
    }
    if let Some(t) = &e.throwable {
        return t.root_cause().short_type().to_owned();
    }
    e.message.clone()
}

/// Значок и подпись одним элементом: «⚠ 3» в счётчиках, строка состояния.
/// Нажимается целиком (`Sense::click`).
pub fn icon_count(ui: &mut Ui, icon: &str, text: &str, size: f32, color: Color32) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(text.to_owned(), FontId::proportional(size), color);
    let icon_side = size + 1.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(icon_side + 4.0 + galley.size().x, galley.size().y.max(icon_side)), Sense::click());
    forma_ui::icons::paint(ui.painter(), egui::Rect::from_center_size(rect.left_center() + vec2(icon_side / 2.0, 0.0), vec2(icon_side, icon_side)), icon, color);
    ui.painter().galley(rect.left_center() + vec2(icon_side + 4.0, -galley.size().y / 2.0), galley, color);
    resp
}

/// Небольшая плашка: цветная точка и подпись. В ряду с переносом плашка не шире ряда:
/// длинная подпись обрезается многоточием и целиком видна во всплывающей подсказке.
pub fn chip(ui: &mut Ui, pal: &Palette, color: Color32, text: &str) {
    let font = FontId::proportional(11.5);
    let max = (ui.max_rect().width() - 24.0).max(24.0);
    let galley = forma_ui::widgets::elide(ui, text, font, pal.text_muted, max);
    let elided = galley.elided;
    let (rect, resp) = ui.allocate_exact_size(galley.size() + vec2(22.0, 6.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(9), color.gamma_multiply(0.14));
    ui.painter().circle_filled(rect.left_center() + vec2(9.0, 0.0), 3.5, color);
    ui.painter().galley(rect.left_top() + vec2(16.0, 3.0), galley, pal.text_muted);
    if elided {
        resp.on_hover_text(text);
    }
}

/// Плашка происхождения. У мода — «Название (id)» и его логотип: свой мод салатовый,
/// чужие — одного цвета и различаются логотипом. У остальных — цветная точка, как у [`chip`].
pub fn origin_tag(ui: &mut Ui, pal: &Palette, origin: &Origin) -> egui::Response {
    let id = crate::mods_view::mod_id(origin);
    let color = match origin {
        Origin::OwnMod => crate::colors::current().tag_own,
        o => origin_color(pal, o),
    };
    let label = origin_label(origin);
    let logo = id.as_deref().and_then(crate::mods_view::logo);
    let font = FontId::proportional(11.5);
    let lead = if logo.is_some() { 21.0 } else { 16.0 };
    let max = (ui.max_rect().width() - lead - 6.0).max(24.0);
    let galley = forma_ui::widgets::elide(ui, label.clone(), font, pal.text_muted, max);
    let elided = galley.elided;
    let (rect, resp) = ui.allocate_exact_size(galley.size() + vec2(lead + 6.0, 6.0), Sense::hover());
    let painter = ui.painter();
    painter.rect(rect, CornerRadius::same(9), color.gamma_multiply(0.14), Stroke::new(1.0, color.gamma_multiply(if id.is_some() { 0.45 } else { 0.0 })), egui::StrokeKind::Inside);
    match &logo {
        Some(tex) => {
            let r = egui::Rect::from_center_size(rect.left_center() + vec2(11.0, 0.0), vec2(14.0, 14.0));
            painter.image(tex.id(), r, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
        }
        None => {
            painter.circle_filled(rect.left_center() + vec2(9.0, 0.0), 3.5, color);
        }
    }
    painter.galley(rect.left_top() + vec2(lead, 3.0), galley, pal.text_muted);
    if elided { resp.on_hover_text(label) } else { resp }
}

/// Подробности записи: плашки, сообщение, объяснение и стек-трейс.
/// `expanded` — какие свёрнутые группы кадров раскрыты.
pub fn entry_details(ui: &mut Ui, pal: &Palette, e: &Entry, libs: Option<&Libraries>, expanded: &mut HashSet<(u64, usize, usize)>, cmds: &mut Vec<Cmd>) {
    ui.horizontal_wrapped(|ui| {
        let lvl = if e.crash { "CRASH" } else { level_tag(e.level) };
        chip(ui, pal, level_color(pal, e.level), lvl);
        chip(ui, pal, pal.text_faint, &system_label(e.system));
        // Две роли: чья проблема и через кого она всплыла.
        origin_tag(ui, pal, &e.origin).on_hover_text(t!("problem-owner-hint"));
        if let Some(via) = &e.via {
            ui.label(RichText::new(t!("problem-via")).size(11.5).color(pal.text_faint));
            origin_tag(ui, pal, via).on_hover_text(t!("problem-via-hint"));
        }
        // Время и поток — одной подписью: длинное имя потока не раздвигает узкую область,
        // а обрезается (целиком оно во всплывающей подсказке).
        let when: Vec<&str> = [e.time.as_deref(), e.thread.as_deref()].into_iter().flatten().collect();
        if !when.is_empty() {
            let text = when.join(" · ");
            let galley = forma_ui::widgets::elide(ui, text.clone(), FontId::proportional(11.5), pal.text_faint, ui.max_rect().width());
            let elided = galley.elided;
            let resp = ui.add(egui::Label::new(galley).selectable(false));
            if elided {
                resp.on_hover_text(text);
            }
        }
    });
    ui.add_space(6.0);

    if let Some(k) = e.known {
        explanation(ui, pal, k);
        ui.add_space(6.0);
    } else if e.noise {
        note(ui, pal, "noise", &t!("noise-title"), &t!("noise-explain"), pal.text_faint);
        ui.add_space(6.0);
    }

    let mono = FontId::monospace(12.5);
    let bad_ids = matches!(e.known, Some(Known::InvalidResourceLocation | Known::InvalidResourcePath));
    ui.add(egui::Label::new(message_job(&e.message, mono.clone(), pal.heading, pal, bad_ids)).wrap());
    for d in &e.details {
        ui.add(egui::Label::new(message_job(d, mono.clone(), pal.text_muted, pal, bad_ids)).wrap());
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
    if let Some(t) = &e.throwable {
        ui.add_space(6.0);
        throwable(ui, pal, e.seq, t, libs, expanded, cmds);
    }
}

/// Id ресурса: `пространство:путь`. Путь — до пробела, кавычки или скобки: в неправильном
/// id бывают и заглавные буквы, и пробелы не бывают.
static RESOURCE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b([a-z][a-z0-9_.-]*):([^\s'\x22,;()\[\]{}<>]+)").expect("регулярное выражение id"));
/// Полное имя класса: `net.minecraft.world.item.Item`, `com.example.Foo$Bar`.
static CLASS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(?:[a-z_][a-z0-9_]*\.){2,}[A-Z][A-Za-z0-9_$]*").expect("регулярное выражение класса"));
/// Файл: путь с расширением ресурса, кода или архива.
static FILE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\w./\\-]+\.(?:json|png|obj|mtl|toml|java|kt|ogg|class|jar|mcmeta|nbt|cfg|properties)\b").expect("регулярное выражение файла"));

/// Сообщение с разметкой: у id ресурса пространство имён окрашено цветом его владельца,
/// классы — цветом происхождения, файлы подчёркнуты. `bad_ids` — подсветить в путях id
/// недопустимые символы (всё, кроме `a-z 0-9 / . _ -`).
pub fn message_job(text: &str, font: FontId, base: Color32, pal: &Palette, bad_ids: bool) -> LayoutJob {
    #[derive(Clone, Copy, PartialEq)]
    enum Kind {
        Resource,
        Class,
        File,
    }
    let mut spans: Vec<(usize, usize, Kind)> = Vec::new();
    for m in RESOURCE.find_iter(text) {
        // `https://…` — адрес, а не id.
        if !m.as_str().contains("://") {
            spans.push((m.start(), m.end(), Kind::Resource));
        }
    }
    for (re, kind) in [(&*CLASS, Kind::Class), (&*FILE, Kind::File)] {
        for m in re.find_iter(text) {
            if !spans.iter().any(|(a, b, _)| m.start() < *b && *a < m.end()) {
                spans.push((m.start(), m.end(), kind));
            }
        }
    }
    spans.sort_by_key(|s| s.0);
    let rules = crate::mods_view::rules();
    let plain = TextFormat::simple(font.clone(), base);
    let mut job = LayoutJob::default();
    let mut at = 0;
    for (start, end, kind) in spans {
        job.append(&text[at..start], 0.0, plain.clone());
        let token = &text[start..end];
        match kind {
            Kind::Resource => {
                let (ns, path) = token.split_once(':').unwrap_or((token, ""));
                let ns_color = origin_color(pal, &rules.namespace(ns));
                job.append(ns, 0.0, TextFormat { color: ns_color, ..plain.clone() });
                job.append(":", 0.0, TextFormat::simple(font.clone(), pal.text_faint));
                if bad_ids {
                    // Недопустимые символы пути — красным и подчёркнутыми.
                    for c in path.chars() {
                        let ok = c.is_ascii_lowercase() || c.is_ascii_digit() || "/._-#".contains(c);
                        let fmt = if ok { plain.clone() } else { TextFormat { color: pal.danger, underline: Stroke::new(1.0, pal.danger), background: pal.danger.gamma_multiply(0.15), ..plain.clone() } };
                        job.append(&c.to_string(), 0.0, fmt);
                    }
                } else {
                    job.append(path, 0.0, plain.clone());
                }
            }
            Kind::Class => {
                let color = origin_color(pal, &rules.class(token.trim_end_matches(['.', '$'])));
                job.append(token, 0.0, TextFormat { color, ..plain.clone() });
            }
            Kind::File => {
                job.append(token, 0.0, TextFormat { underline: Stroke::new(1.0, base.gamma_multiply(0.5)), ..plain.clone() });
            }
        }
        at = end;
    }
    job.append(&text[at..], 0.0, plain);
    job
}

/// Текст и оформление кусков разметки — чтобы приклеить одну разметку к другой.
trait SectionsText {
    fn sections_text(&self) -> Vec<(String, TextFormat)>;
}

impl SectionsText for LayoutJob {
    fn sections_text(&self) -> Vec<(String, TextFormat)> {
        self.sections.iter().map(|s| (self.text[s.byte_range.start.0..s.byte_range.end.0].to_owned(), s.format.clone())).collect()
    }
}

/// Блок с объяснением известной проблемы.
pub fn explanation(ui: &mut Ui, pal: &Palette, k: Known) {
    note(ui, pal, "hint", &known_title(k), &known_explain(k), pal.accent);
}

/// Рамка со значком, заголовком и пояснением в цвете `tint`.
fn note(ui: &mut Ui, pal: &Palette, icon: &str, title: &str, text: &str, tint: Color32) {
    egui::Frame::new()
        .fill(tint.gamma_multiply(0.12))
        .stroke(Stroke::new(1.0, tint.gamma_multiply(0.35)))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(12, 9))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let (r, _) = ui.allocate_exact_size(vec2(16.0, 18.0), Sense::hover());
                forma_ui::icons::paint(ui.painter(), egui::Rect::from_center_size(r.center(), vec2(16.0, 16.0)), icon, tint);
                ui.add(egui::Label::new(RichText::new(title).color(pal.heading).family(forma_ui::fonts::semibold())).wrap());
            });
            ui.add_space(2.0);
            ui.add(egui::Label::new(RichText::new(text).size(13.0).color(pal.text)).wrap());
        });
}

/// Стек-трейс с цепочкой причин. Подряд идущие чужие кадры свёрнуты в одну строку,
/// кадры своего мода яркие и открывают исходник.
pub fn throwable(ui: &mut Ui, pal: &Palette, seq: u64, t: &Throwable, libs: Option<&Libraries>, expanded: &mut HashSet<(u64, usize, usize)>, cmds: &mut Vec<Cmd>) {
    let mono = FontId::monospace(12.0);
    for (ci, cause) in t.chain().enumerate() {
        let prefix = format!("{}{}", if ci > 0 { "Caused by: " } else { "" }, cause.type_name);
        let mut head = LayoutJob::single_section(prefix, TextFormat::simple(mono.clone(), pal.danger));
        if let Some(m) = &cause.message {
            head.append(": ", 0.0, TextFormat::simple(mono.clone(), pal.danger));
            for s in message_job(m, mono.clone(), pal.danger, pal, false).sections_text() {
                head.append(&s.0, 0.0, s.1);
            }
        }
        ui.add(egui::Label::new(head).wrap());
        let mut i = 0;
        while i < cause.frames.len() {
            let f = &cause.frames[i];
            if f.origin == Origin::OwnMod {
                frame_row(ui, pal, f, libs, cmds);
                i += 1;
                continue;
            }
            // Серия чужих кадров.
            let start = i;
            while i < cause.frames.len() && cause.frames[i].origin != Origin::OwnMod {
                i += 1;
            }
            let run = &cause.frames[start..i];
            let key = (seq, ci, start);
            if run.len() <= 2 || expanded.contains(&key) {
                for f in run {
                    frame_row(ui, pal, f, libs, cmds);
                }
            } else {
                let mut origins: Vec<String> = Vec::new();
                for f in run {
                    let l = origin_label(&f.origin);
                    if !origins.contains(&l) {
                        origins.push(l);
                    }
                }
                let text = format!("    ⋯ {}  ({})", t!("frames-hidden", count = run.len()), origins.join(", "));
                let resp = ui.add(egui::Label::new(RichText::new(text).font(mono.clone()).color(pal.text_faint)).sense(Sense::click()));
                if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    expanded.insert(key);
                }
            }
        }
        if cause.omitted > 0 {
            ui.label(RichText::new(format!("    … {}", t!("frames-omitted", count = cause.omitted))).font(mono.clone()).color(pal.text_faint));
        }
    }
}

fn frame_row(ui: &mut Ui, pal: &Palette, f: &Frame, libs: Option<&Libraries>, cmds: &mut Vec<Cmd>) {
    let own = f.origin == Origin::OwnMod;
    // Кадр Minecraft, загрузчика или библиотеки открывается, если нашлись исходники.
    let has_source = !own && libs.is_some_and(|l| l.find_class(&f.class).is_some_and(|(lib, _)| l.libs[lib].sources.is_some()));
    let clickable = own || has_source;
    let color = origin_color(pal, &f.origin);
    let place = match (&f.file, f.line) {
        (Some(file), Some(line)) => format!("{file}:{line}"),
        (Some(file), None) => file.clone(),
        _ => String::new(),
    };
    let text = format!("    at {}.{}({place})", f.class, f.method);
    let font = FontId::monospace(12.0);
    // Длинный кадр переносится по ширине области, а не уходит за её край.
    let galley = ui.painter().layout(text, font, if own { pal.heading } else { pal.text_muted }, (ui.available_width() - 14.0).max(20.0));
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 14.0, galley.size().y + 2.0), if clickable { Sense::click() } else { Sense::hover() });
    let hover = clickable && resp.hovered();
    if hover {
        ui.painter().rect_filled(rect, CornerRadius::same(4), pal.bg_hover);
    }
    ui.painter().rect_filled(egui::Rect::from_min_size(rect.min + vec2(2.0, 2.0), vec2(3.0, rect.height() - 4.0)), CornerRadius::same(1), color);
    ui.painter().galley(rect.min + vec2(10.0, 1.0), galley, pal.text);
    if clickable {
        let hint = if own { t!("frame-open-source") } else { t!("frame-open-library-source") };
        let resp = resp.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(hint);
        if resp.clicked() {
            cmds.push(Cmd::OpenSource { class: f.class.clone(), file: f.file.clone(), line: f.line });
        }
    }
}
