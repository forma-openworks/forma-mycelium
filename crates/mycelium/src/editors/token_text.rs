//! Текст записи с токенами: классы, id ресурсов, файлы, цель mixin, его конфигурация и мод.
//! Токен окрашен цветом владельца и подчёркнут пунктиром — знак «наведи или нажми».
//! Под мышью он подсвечивается (вместе с той же сущностью в заголовке и сообщении),
//! показывает подсказку с фактами о себе, а по клику открывает исходник, конфигурацию или мод.

use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui::{self, Color32, CornerRadius, CursorIcon, FontId, Id, Rect, RichText, Sense, Shape, Stroke, Ui, pos2, text::LayoutJob, text::TextFormat, vec2};
use forma_i18n::t;
use forma_ui::theme::Palette;
use mycelium_hyphae::meta::Format;
use mycelium_hyphae::{ModKind, RunMods};
use mycelium_log::tokens::{class_name, simple_name, tokens};
use mycelium_log::{Field, Known, Role};
use mycelium_project::{Libraries, Origin, OriginRules};

use super::problem_ui::{origin_color, origin_label};
use crate::app::Cmd;

/// Сущность, названная в тексте: роль и полное значение (в заголовке у класса видно
/// короткое имя, а сущность — полное).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Entity {
    pub role: Role,
    pub value: String,
}

/// Кусок текста с оформлением. У кусков одного токена (пакет и имя класса,
/// пространство и путь id) сущность одна.
pub struct Piece {
    pub text: String,
    pub format: TextFormat,
    pub entity: Option<Entity>,
}

impl Piece {
    pub fn plain(text: impl Into<String>, format: TextFormat) -> Self {
        Self { text: text.into(), format, entity: None }
    }
}

/// Что известно вокруг записи: правила происхождения, моды запуска, библиотеки и поля
/// самой записи (владельца mixin знает его конфигурация).
pub struct TokenCx<'a> {
    pub pal: &'a Palette,
    pub libs: Option<&'a Libraries>,
    pub rules: OriginRules,
    pub run: Option<Arc<RunMods>>,
    pub fields: &'a [Field],
}

impl<'a> TokenCx<'a> {
    pub fn new(pal: &'a Palette, libs: Option<&'a Libraries>, fields: &'a [Field]) -> Self {
        Self { pal, libs, rules: crate::mods_view::rules(), run: crate::mods_view::run(), fields }
    }

    fn field(&self, role: Role) -> Option<&str> {
        self.fields.iter().find(|f| f.role == role).map(|f| f.value.as_str())
    }

    /// Чья сущность.
    pub fn owner(&self, e: &Entity) -> Origin {
        match e.role {
            Role::Class | Role::MixinTarget => self.rules.class(&class_name(&e.value)),
            Role::MixinConfig => self.config_owner(&e.value),
            Role::Mixin => match self.field(Role::MixinConfig) {
                Some(config) => self.config_owner(config),
                None => self.field(Role::Mod).map(|m| self.rules.namespace(m)).unwrap_or(Origin::Unknown),
            },
            Role::Mod => self.rules.namespace(&e.value),
            Role::Resource => self.rules.namespace(e.value.split(':').next().unwrap_or("")),
            Role::File => Origin::Unknown,
        }
    }

    fn config_owner(&self, config: &str) -> Origin {
        self.rules.mixin_config(config).or_else(|| self.field(Role::Mod).map(|m| self.rules.namespace(m))).unwrap_or(Origin::Unknown)
    }

    /// Цвет сущности — цвет её владельца; у файла — цвет текста вокруг.
    pub fn color(&self, e: &Entity, base: Color32) -> Color32 {
        match e.role {
            Role::File => base,
            _ => origin_color(self.pal, &self.owner(e)),
        }
    }
}

/// Куски сообщения: токены окрашены цветом владельца, у классов пакет приглушён.
/// `bad_ids` — подсветить в путях id недопустимые символы (всё, кроме `a-z 0-9 / . _ -`).
pub fn message_pieces(text: &str, cx: &TokenCx, font: &FontId, base: Color32, bad_ids: bool) -> Vec<Piece> {
    let plain = TextFormat::simple(font.clone(), base);
    let mut out = Vec::new();
    let mut at = 0;
    for tok in tokens(text, cx.fields) {
        if tok.range.start > at {
            out.push(Piece::plain(&text[at..tok.range.start], plain.clone()));
        }
        let value = &text[tok.range.clone()];
        // Класс через `/` и через точки — одна сущность: подсвечиваются вместе.
        let entity = Entity { role: tok.role, value: if tok.role.is_class() { class_name(value) } else { value.to_owned() } };
        token_pieces(&mut out, value, &entity, cx, font, base, bad_ids);
        at = tok.range.end;
    }
    if at < text.len() {
        out.push(Piece::plain(&text[at..], plain));
    }
    out
}

fn token_pieces(out: &mut Vec<Piece>, text: &str, entity: &Entity, cx: &TokenCx, font: &FontId, base: Color32, bad_ids: bool) {
    let color = cx.color(entity, base);
    let piece = |text: &str, format: TextFormat| Piece { text: text.to_owned(), format, entity: Some(entity.clone()) };
    let fmt = |c: Color32| TextFormat::simple(font.clone(), c);
    match entity.role {
        role if role.is_class() => {
            // Пакет приглушён, имя ярко: так класс находится взглядом.
            let simple = simple_name(text);
            let package = &text[..text.len() - simple.len()];
            if !package.is_empty() {
                out.push(piece(package, fmt(color.gamma_multiply(0.6))));
            }
            out.push(piece(simple, fmt(color)));
        }
        Role::Resource => {
            let (ns, path) = text.split_once(':').unwrap_or((text, ""));
            out.push(piece(ns, fmt(color)));
            out.push(piece(":", fmt(cx.pal.text_faint)));
            if !bad_ids {
                out.push(piece(path, fmt(base)));
                return;
            }
            // Недопустимые символы пути — красным и подчёркнутыми.
            let bad = TextFormat { underline: Stroke::new(1.0, cx.pal.danger), background: cx.pal.danger.gamma_multiply(0.15), ..fmt(cx.pal.danger) };
            let mut run = String::new();
            let mut run_bad = false;
            for c in path.chars() {
                let is_bad = !(c.is_ascii_lowercase() || c.is_ascii_digit() || "/._-#".contains(c));
                if is_bad != run_bad && !run.is_empty() {
                    out.push(piece(&run, if run_bad { bad.clone() } else { fmt(base) }));
                    run.clear();
                }
                run_bad = is_bad;
                run.push(c);
            }
            if !run.is_empty() {
                out.push(piece(&run, if run_bad { bad } else { fmt(base) }));
            }
        }
        _ => out.push(piece(text, fmt(color))),
    }
}

/// Части заголовка с подробностями: текст и, у подстановок, роль поля.
/// «Mixin target {target} was not found» → «Mixin target », «KeyBindsList$KeyEntry», « was not found».
/// У классов в заголовке — короткое имя. Не хватает полей — `None`.
pub fn headline(known: Known, fields: &[Field]) -> Option<Vec<(String, Option<Role>)>> {
    let roles = known.headline_roles();
    if roles.is_empty() {
        return None;
    }
    let value = |role: Role| fields.iter().find(|f| f.role == role).map(|f| f.value.as_str());
    // Подстановки помечены символами из частной области Unicode — по ним текст режется на части.
    let mut args = forma_i18n::FluentArgs::new();
    for r in roles {
        value(*r)?;
        args.set(r.key(), format!("\u{E000}{}\u{E001}", r.key()));
    }
    let text = forma_i18n::tr_args(&format!("known-{}-headline", known.key()), &args);
    let mut out = Vec::new();
    for (i, part) in text.split('\u{E000}').enumerate() {
        let (key, rest) = if i == 0 { ("", part) } else { part.split_once('\u{E001}').unwrap_or(("", part)) };
        if let Some(role) = roles.iter().copied().find(|r| r.key() == key) {
            let v = value(role).unwrap_or_default();
            out.push((if role.is_class() { simple_name(v) } else { v }.to_owned(), Some(role)));
        }
        if !rest.is_empty() {
            out.push((rest.to_owned(), None));
        }
    }
    Some(out)
}

/// Куски заголовка с подробностями: подстановки — токены цвета владельца.
pub fn headline_pieces(known: Known, cx: &TokenCx, font: &FontId, color: Color32) -> Option<Vec<Piece>> {
    let parts = headline(known, cx.fields)?;
    Some(
        parts
            .into_iter()
            .map(|(text, role)| match role {
                Some(role) => {
                    let entity = Entity { role, value: cx.field(role).unwrap_or_default().to_owned() };
                    Piece { format: TextFormat::simple(font.clone(), cx.color(&entity, color)), text, entity: Some(entity) }
                }
                None => Piece::plain(text, TextFormat::simple(font.clone(), color)),
            })
            .collect(),
    )
}

/// Сущность под мышью в области и кадр, когда её видели: так та же сущность
/// подсвечивается в заголовке и в сообщении.
#[derive(Clone)]
struct Hovered {
    frame: u64,
    entity: Entity,
}

/// Рисует куски одним текстом с переносом. Текст выделяется мышью, как обычная подпись;
/// токены подчёркнуты пунктиром, подсвечиваются, показывают подсказку и по клику
/// выполняют своё действие. `scope` — область, внутри которой подсвечивается одна сущность.
pub fn show(ui: &mut Ui, scope: Id, pieces: Vec<Piece>, cx: &TokenCx, cmds: &mut Vec<Cmd>) -> egui::Response {
    let mut job = LayoutJob::default();
    let mut entities: Vec<Entity> = Vec::new();
    // Символы текста → сущность. Соседние куски одного токена склеены.
    let mut spans: Vec<(Range<usize>, usize)> = Vec::new();
    let mut chars = 0;
    for p in pieces {
        let n = p.text.chars().count();
        if let Some(e) = p.entity {
            let i = entities.iter().position(|x| *x == e).unwrap_or_else(|| {
                entities.push(e);
                entities.len() - 1
            });
            match spans.last_mut() {
                Some((r, j)) if *j == i && r.end == chars => r.end += n,
                _ => spans.push((chars..chars + n, i)),
            }
        }
        job.append(&p.text, 0.0, p.format);
        chars += n;
    }

    let (pos, galley, resp) = egui::Label::new(job).wrap().layout_in_ui(ui);
    // Подсветка и пунктир — под текстом: место под них занимается до текста.
    let under = ui.painter().add(Shape::Noop);
    egui::text_selection::LabelSelectionState::label_text_selection(ui, &resp, pos, galley.clone(), cx.pal.text, Stroke::NONE);

    // Прямоугольники токенов по строкам: перенесённый токен — несколько прямоугольников.
    let mut rects: Vec<(usize, Rect)> = Vec::new();
    let mut c = 0;
    for row in &galley.rows {
        for g in &row.row.glyphs {
            if let Some(si) = spans.iter().position(|(r, _)| r.contains(&c)) {
                let x = row.pos.x + g.pos.x;
                let r = Rect::from_min_max(pos2(x, row.min_y()), pos2(x + g.advance_width, row.max_y())).translate(pos.to_vec2());
                match rects.last_mut() {
                    Some((s, prev)) if *s == si && (prev.min.y - r.min.y).abs() < 0.5 => *prev = prev.union(r),
                    _ => rects.push((si, r)),
                }
            }
            c += 1;
        }
        if row.ends_with_newline {
            c += 1;
        }
    }

    let ctx = ui.ctx().clone();
    let frame = ctx.cumulative_frame_nr();
    let key = scope.with("mycelium-hovered-entity");
    let shared = ui.data(|d| d.get_temp::<Hovered>(key)).filter(|h| h.frame + 1 >= frame).map(|h| h.entity);
    let mut hovered: Option<usize> = None;
    let mut shapes = Vec::new();
    for (k, (si, rect)) in rects.iter().enumerate() {
        let ei = spans[*si].1;
        let entity = &entities[ei];
        // Клик — токену, протяжка — подписи под ним: текст по-прежнему выделяется.
        let r = ui.interact(*rect, resp.id.with(("token", k)), Sense::click());
        if r.hovered() {
            hovered = Some(ei);
            if card(&ctx, cx, entity).action.go().is_some() {
                ctx.set_cursor_icon(CursorIcon::PointingHand);
            }
        }
        if r.clicked()
            && let Some(go) = card(&ctx, cx, entity).action.go()
        {
            cmds.push(go.cmd());
        }
        egui::containers::Tooltip::for_enabled(&r).width(330.0).show(|ui| card_ui(ui, cx.pal, &card(&ctx, cx, entity)));

        let color = cx.color(entity, cx.pal.text);
        if hovered == Some(ei) || shared.as_ref() == Some(entity) {
            shapes.push(Shape::rect_filled(rect.expand2(vec2(1.0, 0.0)), CornerRadius::same(3), color.gamma_multiply(0.2)));
        }
        let y = rect.bottom() - 1.0;
        shapes.extend(Shape::dotted_line(&[pos2(rect.left() + 1.0, y), pos2(rect.right(), y)], color.gamma_multiply(0.75), 3.0, 0.6));
    }
    ui.painter().set(under, Shape::Vec(shapes));
    if let Some(ei) = hovered {
        ui.data_mut(|d| d.insert_temp(key, Hovered { frame, entity: entities[ei].clone() }));
    }
    resp
}

/// Что сделать по клику на токен.
#[derive(Clone, Debug)]
enum Go {
    /// Исходник класса (своего мода или библиотеки).
    Source(String),
    /// Файл мода: у своего — из ресурсов проекта, у чужого — из его jar.
    ModFile { id: String, path: String },
    /// Мод во вкладке «Моды запуска».
    Mod(String),
    File(PathBuf),
}

impl Go {
    fn cmd(&self) -> Cmd {
        match self.clone() {
            Go::Source(class) => Cmd::OpenSource { class, file: None, line: None },
            Go::ModFile { id, path } => Cmd::OpenModFile { id, path },
            Go::Mod(id) => Cmd::ShowMod(id),
            Go::File(path) => Cmd::OpenFile { path, line: None },
        }
    }
}

#[derive(Clone, Debug)]
enum Action {
    None,
    Run { icon: &'static str, label: String, go: Go },
    /// Действия пока нет — объясняем, когда оно появится.
    Later(String),
}

impl Action {
    fn go(&self) -> Option<&Go> {
        match self {
            Action::Run { go, .. } => Some(go),
            _ => None,
        }
    }
}

/// Подсказка о токене: роль, значение, факты, пояснение и действие.
#[derive(Clone, Debug)]
struct Card {
    label: String,
    value: String,
    color: Color32,
    /// Факты; `true` — предупреждение (например, такого класса в этой версии нет).
    facts: Vec<(String, bool)>,
    hint: Option<String>,
    action: Action,
}

/// Подсказка о сущности. Считается по библиотекам и модам запуска, поэтому запоминается,
/// пока они те же: мышь над токеном держат много кадров подряд.
fn card(ctx: &egui::Context, cx: &TokenCx, e: &Entity) -> Card {
    let stamp = (cx.libs.map(|l| l as *const Libraries as usize), cx.run.as_ref().map(|r| Arc::as_ptr(r) as usize), cx.fields);
    let id = Id::new(("mycelium-token-card", e, stamp));
    if let Some(c) = ctx.data(|d| d.get_temp::<Card>(id)) {
        return c;
    }
    let c = make_card(cx, e);
    ctx.data_mut(|d| d.insert_temp(id, c.clone()));
    c
}

fn make_card(cx: &TokenCx, e: &Entity) -> Card {
    let owner = cx.owner(e);
    let mut c = Card {
        label: forma_i18n::tr(&format!("token-{}", e.role.key())),
        value: e.value.clone(),
        color: cx.color(e, cx.pal.text),
        facts: Vec::new(),
        hint: None,
        action: Action::None,
    };
    let run = cx.run.as_deref();
    match e.role {
        Role::Class | Role::MixinTarget => {
            let name = class_name(&e.value);
            c.value = name.clone();
            c.facts.push((origin_label(&owner), false));
            let mut hints = Vec::new();
            if e.role == Role::MixinTarget {
                hints.push(t!("token-target-hint"));
            }
            if let Some((outer, inner)) = simple_name(&name).split_once('$') {
                hints.push(t!("token-nested-hint", outer = outer.to_owned(), inner = inner.to_owned()));
            }
            c.hint = (!hints.is_empty()).then(|| hints.join(" "));
            // Minecraft и загрузчик библиотеки знают целиком — по ним видно, есть ли класс.
            let mut open = name.clone();
            let platform = matches!(owner, Origin::Minecraft | Origin::NeoForge | Origin::Fabric | Origin::Forge);
            if let (true, Some(libs)) = (platform, cx.libs)
                && !libs.has_class(&name)
            {
                match libs.classes_named(&name).into_iter().next() {
                    Some(moved) => {
                        c.facts.push((t!("token-class-moved-hint", class = moved.clone()), true));
                        open = moved;
                    }
                    None => c.facts.push((t!("token-class-absent-hint"), true)),
                }
            }
            c.action = class_action(cx, &open, &owner);
        }
        Role::MixinConfig => {
            match run.and_then(|r| r.mixin_config(&e.value)) {
                Some((m, config)) => {
                    c.facts.push((crate::mods_view::mod_label(&m.id), false));
                    if let Some(f) = m.format {
                        c.facts.push((t!("token-config-declared-hint", file = declared_in(f)), false));
                    }
                    if let Some(config) = config.filter(|c| !c.package.is_empty()) {
                        c.facts.push((t!("token-config-package", package = config.package.clone()), false));
                    }
                    let own = m.kind == ModKind::Own;
                    if own || (m.jar.is_some() && m.inner.is_none()) {
                        let label = if own { t!("token-open-file") } else { t!("token-open-from-jar") };
                        c.action = Action::Run { icon: "file-json", label, go: Go::ModFile { id: m.id.clone(), path: e.value.clone() } };
                    }
                }
                None => c.facts.push((origin_label(&owner), false)),
            }
            c.hint = Some(t!("token-config-hint"));
        }
        Role::Mixin => {
            let config = cx.field(Role::MixinConfig).and_then(|n| run.and_then(|r| r.mixin_config(n))).and_then(|(_, c)| c);
            let full = config.map(|cfg| cfg.full_name(&e.value));
            if let Some(full) = &full {
                c.facts.push((full.clone(), false));
            }
            if let Some(side) = config.and_then(|cfg| cfg.side_of(&e.value)) {
                c.facts.push((forma_i18n::tr(&format!("token-mixin-side-{}-hint", side.key())), false));
            }
            c.hint = Some(t!("token-mixin-hint"));
            c.action = match (&owner, full) {
                (Origin::OwnMod, Some(full)) => Action::Run { icon: "file-code", label: t!("frame-open-source"), go: Go::Source(full) },
                (Origin::OwnMod, None) => Action::None,
                _ => Action::Later(t!("token-mixin-later-hint")),
            };
        }
        Role::Mod => {
            match run.and_then(|r| r.get(&e.value)) {
                Some(m) => {
                    let line = if m.version.is_empty() { m.name.clone() } else { format!("{} {}", m.name, m.version) };
                    c.facts.push((line, false));
                    c.action = Action::Run { icon: "editor-mods", label: t!("token-show-mod"), go: Go::Mod(m.id.clone()) };
                }
                None => c.facts.push((origin_label(&owner), false)),
            }
            c.hint = Some(t!("token-mod-hint"));
        }
        Role::Resource => {
            c.facts.push((origin_label(&owner), false));
            c.hint = Some(t!("token-resource-hint"));
        }
        Role::File => {
            let path = PathBuf::from(&e.value);
            if path.is_absolute() && path.is_file() {
                c.action = Action::Run { icon: "file", label: t!("token-open-file"), go: Go::File(path) };
            }
        }
    }
    c
}

/// Открыть исходник класса: свой — из проекта, Minecraft и библиотеки — если есть исходники.
fn class_action(cx: &TokenCx, class: &str, owner: &Origin) -> Action {
    let has_sources = || cx.libs.and_then(|l| l.find_class(class).map(|(lib, _)| l.libs[lib].sources.is_some())).unwrap_or(false);
    let label = match owner {
        Origin::OwnMod => t!("frame-open-source"),
        _ if !has_sources() => return Action::None,
        Origin::Minecraft => t!("token-open-minecraft-source"),
        _ => t!("frame-open-library-source"),
    };
    Action::Run { icon: "file-code", label, go: Go::Source(class.to_owned()) }
}

/// Где подключена конфигурация mixin.
fn declared_in(format: Format) -> &'static str {
    match format {
        Format::NeoForge => "neoforge.mods.toml · [[mixins]]",
        Format::Forge => "mods.toml · [[mixins]]",
        Format::Fabric => "fabric.mod.json · mixins",
        Format::Quilt => "quilt.mod.json · mixin",
    }
}

fn card_ui(ui: &mut Ui, pal: &Palette, c: &Card) {
    ui.spacing_mut().item_spacing.y = 4.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let (r, _) = ui.allocate_exact_size(vec2(8.0, 14.0), Sense::hover());
        ui.painter().rect_filled(Rect::from_center_size(r.center(), vec2(8.0, 8.0)), CornerRadius::same(2), c.color);
        ui.label(RichText::new(&c.label).size(12.0).color(pal.text_muted));
    });
    ui.add(egui::Label::new(RichText::new(&c.value).font(FontId::monospace(12.5)).color(c.color)).wrap());
    for (fact, warn) in &c.facts {
        ui.add(egui::Label::new(RichText::new(fact).size(12.0).color(if *warn { pal.warning } else { pal.text_muted })).wrap());
    }
    if let Some(hint) = &c.hint {
        ui.add(egui::Label::new(RichText::new(hint).size(13.0).color(pal.text)).wrap());
    }
    let (icon, label, faint) = match &c.action {
        Action::None => return,
        Action::Run { icon, label, .. } => (*icon, label, false),
        Action::Later(text) => ("loading", text, true),
    };
    ui.add_space(2.0);
    ui.separator();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let tint = if faint { pal.text_faint } else { pal.text };
        let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
        forma_ui::icons::paint(ui.painter(), r, icon, tint);
        ui.add(egui::Label::new(RichText::new(label).size(12.5).color(tint)).wrap());
        if !faint {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                egui::Frame::new().fill(pal.bg_hover).corner_radius(4).inner_margin(egui::Margin::symmetric(5, 1)).show(ui, |ui| {
                    ui.label(RichText::new(t!("token-click")).size(11.0).color(pal.text_muted));
                });
            });
        }
    });
}
