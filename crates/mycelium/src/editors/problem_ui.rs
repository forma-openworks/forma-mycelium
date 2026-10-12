//! Общие элементы консоли и окна исключений: цвета происхождения, подписи систем и уровней,
//! объяснения известных проблем и стек-трейс, в котором чужие кадры свёрнуты.

use std::collections::HashSet;

use eframe::egui::{self, Color32, CornerRadius, CursorIcon, FontId, RichText, Sense, Stroke, Ui, text::TextFormat, vec2};
use forma_i18n::t;
use mycelium_log::{Entry, Frame, Known, Level, System, Throwable};
use mycelium_project::{Libraries, Origin};

use super::token_text::{self, Piece, TokenCx};
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
        Known::MixinTargetMissing => t!("known-mixin-target-title"),
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
        Known::MixinTargetMissing => t!("known-mixin-target-explain"),
    }
}

/// Короткое название проблемы: известная (с подробностями, если они разобраны),
/// тип исключения или текст сообщения.
pub fn entry_title(e: &Entry) -> String {
    if let Some(k) = e.known {
        return match token_text::headline(k, &e.fields) {
            Some(parts) => parts.into_iter().map(|(text, _)| text).collect(),
            None => known_title(k),
        };
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

/// Плашка происхождения. У мода — логотип, название и id отдельной моноширинной плашкой:
/// свой мод салатовый, чужие — одного цвета и различаются логотипом. У остальных — цветная
/// точка и подпись.
pub fn origin_tag(ui: &mut Ui, pal: &Palette, origin: &Origin) -> egui::Response {
    let id = crate::mods_view::mod_id(origin);
    let color = match origin {
        Origin::OwnMod => crate::colors::current().tag_own,
        o => origin_color(pal, o),
    };
    // Название — отдельно от id: id показывается, только если название другое.
    let name = id.as_deref().and_then(crate::mods_view::mod_name);
    let label = match (&id, &name) {
        (_, Some(name)) => name.clone(),
        (Some(id), None) => id.clone(),
        (None, _) => origin_label(origin),
    };
    let badge = name.as_ref().and(id.as_ref()).map(|id| ui.painter().layout_no_wrap(id.clone(), FontId::monospace(10.5), pal.text_faint));
    let badge_w = badge.as_ref().map_or(0.0, |b| b.size().x + 12.0);
    let logo = id.as_deref().and_then(crate::mods_view::logo);
    let font = FontId::proportional(12.0);
    let lead = if logo.is_some() { 24.0 } else { 18.0 };
    let max = (ui.max_rect().width() - lead - badge_w - 10.0).max(24.0);
    let galley = forma_ui::widgets::elide(ui, label.clone(), font, pal.text, max);
    let elided = galley.elided;
    let (rect, resp) = ui.allocate_exact_size(galley.size() + vec2(lead + badge_w + 10.0, 8.0), Sense::hover());
    let painter = ui.painter();
    painter.rect(rect, CornerRadius::same(11), color.gamma_multiply(0.14), Stroke::new(1.0, color.gamma_multiply(if id.is_some() { 0.5 } else { 0.0 })), egui::StrokeKind::Inside);
    match &logo {
        Some(tex) => {
            let r = egui::Rect::from_center_size(rect.left_center() + vec2(12.0, 0.0), vec2(15.0, 15.0));
            painter.image(tex.id(), r, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
        }
        None => {
            painter.circle_filled(rect.left_center() + vec2(10.0, 0.0), 3.5, color);
        }
    }
    let text_w = galley.size().x;
    painter.galley(rect.left_top() + vec2(lead, 4.0), galley, pal.text);
    if let Some(b) = badge {
        let r = egui::Rect::from_min_size(egui::pos2(rect.left() + lead + text_w + 6.0, rect.center().y - b.size().y / 2.0 - 1.0), b.size() + vec2(8.0, 2.0));
        painter.rect_filled(r, CornerRadius::same(4), pal.bg_deep.gamma_multiply(0.7));
        painter.galley(r.min + vec2(4.0, 1.0), b, pal.text_faint);
    }
    if elided { resp.on_hover_text(origin_label(origin)) } else { resp }
}

/// Область подсветки сущностей записи: заголовок, сообщение и стек-трейс одной записи.
pub fn entry_scope(e: &Entry) -> egui::Id {
    egui::Id::new("mycelium-entry").with(e.seq)
}

/// Стек-трейс с цепочкой причин. Подряд идущие чужие кадры свёрнуты в одну строку,
/// кадры своего мода яркие и открывают исходник.
pub fn throwable(ui: &mut Ui, cx: &TokenCx, scope: egui::Id, seq: u64, t: &Throwable, expanded: &mut HashSet<(u64, usize, usize)>, cmds: &mut Vec<Cmd>) {
    let (pal, libs) = (cx.pal, cx.libs);
    let mono = FontId::monospace(12.0);
    for (ci, cause) in t.chain().enumerate() {
        let prefix = format!("{}{}", if ci > 0 { "Caused by: " } else { "" }, cause.type_name);
        let red = TextFormat::simple(mono.clone(), pal.danger);
        let mut head = vec![Piece::plain(prefix, red.clone())];
        if let Some(m) = &cause.message {
            head.push(Piece::plain(": ", red));
            head.extend(token_text::message_pieces(m, cx, &mono, pal.danger, false));
        }
        token_text::show(ui, scope, head, cx, cmds);
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
