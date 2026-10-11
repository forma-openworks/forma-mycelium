//! Общие элементы консоли и окна исключений: цвета происхождения, подписи систем и уровней,
//! объяснения известных проблем и стек-трейс, в котором чужие кадры свёрнуты.

use std::collections::HashSet;

use eframe::egui::{self, Color32, CornerRadius, CursorIcon, FontId, RichText, Sense, Stroke, Ui, vec2};
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

pub fn origin_label(origin: &Origin) -> String {
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

/// Подробности записи: плашки, сообщение, объяснение и стек-трейс.
/// `expanded` — какие свёрнутые группы кадров раскрыты.
pub fn entry_details(ui: &mut Ui, pal: &Palette, e: &Entry, libs: Option<&Libraries>, expanded: &mut HashSet<(u64, usize, usize)>, cmds: &mut Vec<Cmd>) {
    ui.horizontal_wrapped(|ui| {
        let lvl = if e.crash { "CRASH" } else { level_tag(e.level) };
        chip(ui, pal, level_color(pal, e.level), lvl);
        chip(ui, pal, pal.text_faint, &system_label(e.system));
        chip(ui, pal, origin_color(pal, &e.origin), &origin_label(&e.origin));
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
        note(ui, pal, &format!("🍃 {}", t!("noise-title")), &t!("noise-explain"), pal.text_faint);
        ui.add_space(6.0);
    }

    let mono = FontId::monospace(12.5);
    ui.add(egui::Label::new(RichText::new(&e.message).font(mono.clone()).color(pal.heading)).wrap());
    for d in &e.details {
        ui.add(egui::Label::new(RichText::new(d).font(mono.clone()).color(pal.text_muted)).wrap());
    }
    if let Some(loc) = &e.location {
        let text = format!("↪ {}:{}", loc.file, loc.line);
        if ui.add(egui::Label::new(RichText::new(text).font(mono.clone()).color(pal.accent)).sense(Sense::click())).on_hover_cursor(CursorIcon::PointingHand).clicked() {
            cmds.push(Cmd::OpenFile { path: loc.file.clone().into(), line: Some(loc.line) });
        }
    }
    if let Some(t) = &e.throwable {
        ui.add_space(6.0);
        throwable(ui, pal, e.seq, t, libs, expanded, cmds);
    }
}

/// Блок с объяснением известной проблемы.
pub fn explanation(ui: &mut Ui, pal: &Palette, k: Known) {
    note(ui, pal, &format!("💡 {}", known_title(k)), &known_explain(k), pal.accent);
}

/// Рамка с заголовком и пояснением в цвете `tint`.
fn note(ui: &mut Ui, pal: &Palette, title: &str, text: &str, tint: Color32) {
    egui::Frame::new()
        .fill(tint.gamma_multiply(0.12))
        .stroke(Stroke::new(1.0, tint.gamma_multiply(0.35)))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(12, 9))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).color(pal.heading).family(forma_ui::fonts::semibold()));
            ui.add_space(2.0);
            ui.add(egui::Label::new(RichText::new(text).size(13.0).color(pal.text)).wrap());
        });
}

/// Стек-трейс с цепочкой причин. Подряд идущие чужие кадры свёрнуты в одну строку,
/// кадры своего мода яркие и открывают исходник.
pub fn throwable(ui: &mut Ui, pal: &Palette, seq: u64, t: &Throwable, libs: Option<&Libraries>, expanded: &mut HashSet<(u64, usize, usize)>, cmds: &mut Vec<Cmd>) {
    let mono = FontId::monospace(12.0);
    for (ci, cause) in t.chain().enumerate() {
        let head = match &cause.message {
            Some(m) => format!("{}{}: {m}", if ci > 0 { "Caused by: " } else { "" }, cause.type_name),
            None => format!("{}{}", if ci > 0 { "Caused by: " } else { "" }, cause.type_name),
        };
        ui.add(egui::Label::new(RichText::new(head).font(mono.clone()).color(pal.danger)).wrap());
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
