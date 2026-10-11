//! Подсказки языкового сервера в редакторе кода: волнистые подчёркивания ошибок,
//! метки на полях, описания при наведении, список автодополнения и переход к определению.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, CursorIcon, FontId, Frame, Id, Key, Margin, Modifiers, Order, Pos2, Rect, RichText, Sense, Shape, Stroke, TextEdit, Ui,
    text::{CCursor, CCursorRange},
    text_edit::TextEditOutput,
    vec2,
};
use forma_i18n::t;

use super::code::Document;
use crate::actions;
use crate::lsp::{self, Completion, CompletionItem, JavaLsp, Severity};
use forma_ui::theme::Palette;

/// Сколько вариантов показывать в списке без прокрутки.
const VISIBLE_ITEMS: usize = 10;
const ITEM_H: f32 = 22.0;
/// Через сколько секунд неподвижного указателя запрашивать описание.
const HOVER_DELAY: f32 = 0.5;

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

/// Начало слова, в котором стоит курсор.
pub fn word_start(text: &str, at: usize) -> usize {
    let chars: Vec<char> = text.chars().take(at).collect();
    let mut i = chars.len();
    while i > 0 && is_ident(chars[i - 1]) {
        i -= 1;
    }
    i
}

/// Слово вокруг символа `at`: начало и конец.
fn word_range(text: &str, at: usize) -> (usize, usize) {
    let start = word_start(text, at);
    let end = start + text.chars().skip(start).take_while(|c| is_ident(*c)).count();
    (start, end.max(start))
}

fn cursor_of(ctx: &egui::Context, id: Id) -> Option<usize> {
    TextEdit::load_state(ctx, id)?.cursor.char_range().map(|r| r.primary.index.0)
}

fn set_cursor(ctx: &egui::Context, id: Id, at: usize) {
    if let Some(mut state) = TextEdit::load_state(ctx, id) {
        state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(at))));
        state.store(ctx, id);
    }
}

/// Заменяет символы `[from, to)` на `insert`.
fn replace_chars(text: &mut String, from: usize, to: usize, insert: &str) {
    let byte = |ci: usize| text.char_indices().nth(ci).map(|(b, _)| b).unwrap_or(text.len());
    let (a, b) = (byte(from), byte(to.max(from)));
    text.replace_range(a..b, insert);
}

/// Правки от сервера (импорты после автодополнения): применяются с конца файла,
/// курсор сдвигается на вставленное выше него.
pub fn apply_server_edits(ctx: &egui::Context, id: Id, doc: &mut Document, lsp: &mut JavaLsp, now: f64) {
    let mine: Vec<_> = lsp.edits.iter().filter(|(p, _)| *p == doc.path).flat_map(|(_, e)| e.clone()).collect();
    if mine.is_empty() {
        return;
    }
    lsp.edits.retain(|(p, _)| *p != doc.path);
    let mut ranges: Vec<(usize, usize, String)> =
        mine.into_iter().map(|e| (lsp::char_index(&doc.text, e.start.0, e.start.1), lsp::char_index(&doc.text, e.end.0, e.end.1), e.text)).collect();
    ranges.sort_by(|a, b| b.0.cmp(&a.0));
    let mut cursor = cursor_of(ctx, id).unwrap_or(0);
    for (from, to, text) in ranges {
        replace_chars(&mut doc.text, from, to, &text);
        let inserted = text.chars().count();
        if to <= cursor {
            cursor = cursor - (to - from) + inserted;
        } else if from < cursor {
            cursor = from + inserted;
        }
    }
    set_cursor(ctx, id, cursor);
    doc.mark_edited(now);
}

/// Варианты, подходящие к уже набранной части слова.
pub fn filtered(c: &Completion, typed: &str) -> Vec<usize> {
    let typed = typed.to_lowercase();
    let mut first: Vec<usize> = Vec::new();
    let mut rest: Vec<usize> = Vec::new();
    for (i, item) in c.items.iter().enumerate() {
        let f = item.filter.to_lowercase();
        if f.starts_with(&typed) {
            first.push(i);
        } else if typed.chars().count() >= 2 && f.contains(&typed) {
            rest.push(i);
        }
    }
    first.extend(rest);
    first
}

fn typed_part(doc: &Document, c: &Completion, cursor: usize) -> String {
    doc.text.chars().skip(c.anchor).take(cursor.saturating_sub(c.anchor)).collect()
}

/// Клавиши до того, как их увидит поле ввода: стрелки и Enter в списке, автодополнение
/// и переход к определению (их сочетания — из раскладки клавиш).
/// Возвращает `true`, если Enter или Tab уже обработаны.
pub fn handle_keys(ctx: &egui::Context, id: Id, doc: &mut Document, lsp: &mut JavaLsp, keys: super::Keys, now: f64) -> bool {
    let cursor = cursor_of(ctx, id).unwrap_or(0);
    if ctx.input_mut(|i| forma_actions::consume(i, keys.actions, keys.keymap, actions::COMPLETION)) {
        lsp.request_completion(doc, cursor, word_start(&doc.text, cursor));
    }
    let definition = ctx.input_mut(|i| forma_actions::consume(i, keys.actions, keys.keymap, actions::GOTO_DEFINITION));
    if definition {
        lsp.request_definition(doc, cursor);
    }

    let Some(c) = lsp.completion.as_ref().filter(|c| c.doc == doc.path) else { return false };
    let list = filtered(c, &typed_part(doc, c, cursor));
    if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
        lsp.completion = None;
        return false;
    }
    if list.is_empty() {
        return false;
    }
    let len = list.len();
    let pos = list.iter().position(|&i| i == c.selected).unwrap_or(0);
    let down = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown));
    let up = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp));
    let accept = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter) || i.consume_key(Modifiers::NONE, Key::Tab));
    let new_pos = if down {
        (pos + 1) % len
    } else if up {
        (pos + len - 1) % len
    } else {
        pos
    };
    if let Some(c) = lsp.completion.as_mut() {
        c.selected = list[new_pos];
    }
    if accept {
        accept_item(ctx, id, doc, lsp, list[new_pos], now);
        return true;
    }
    false
}

/// Вставляет выбранный вариант и просит у сервера импорт для него.
fn accept_item(ctx: &egui::Context, id: Id, doc: &mut Document, lsp: &mut JavaLsp, index: usize, now: f64) {
    let Some(c) = lsp.completion.take() else { return };
    let Some(item) = c.items.get(index).cloned() else { return };
    let cursor = cursor_of(ctx, id).unwrap_or(c.anchor);
    let start = item.replace_from.map(|(l, ch)| lsp::char_index(&doc.text, l, ch)).filter(|s| *s <= cursor).unwrap_or(c.anchor);
    // Без сниппетов сервер присылает только имя метода — скобки добавляем сами, как IDEA:
    // курсор внутрь, если у метода есть параметры, иначе за скобки.
    let mut insert = item.insert.clone();
    let mut caret_back = 0;
    let method = matches!(item.kind, 2 | 3 | 4);
    let next = doc.text.chars().nth(cursor);
    if method && !insert.contains('(') && next != Some('(') {
        let has_params = item.label.split_once('(').is_some_and(|(_, rest)| !rest.starts_with(')'));
        insert.push_str("()");
        if has_params {
            caret_back = 1;
        }
    }
    replace_chars(&mut doc.text, start, cursor, &insert);
    set_cursor(ctx, id, start + insert.chars().count() - caret_back);
    doc.mark_edited(now);
    lsp.resolve(&doc.path.clone(), &item);
}

/// После ввода: точка открывает список, набор уточняет его, уход курсора закрывает.
pub fn after_input(doc: &Document, lsp: &mut JavaLsp, output: &TextEditOutput, changed: bool) {
    let Some(cursor) = output.cursor_range.map(|r| r.primary.index.0) else { return };
    let before = doc.text.chars().nth(cursor.wrapping_sub(1));
    if changed && before == Some('.') {
        lsp.request_completion(doc, cursor, cursor);
        return;
    }
    let Some(c) = lsp.completion.as_ref().filter(|c| c.doc == doc.path) else { return };
    let typed = typed_part(doc, c, cursor);
    if cursor < c.anchor || !typed.chars().all(is_ident) {
        lsp.completion = None;
    } else if changed && c.incomplete && !c.loading {
        let anchor = c.anchor;
        lsp.request_completion(doc, cursor, anchor);
    }
}

fn severity_color(pal: &Palette, s: Severity) -> Color32 {
    match s {
        Severity::Error => pal.editor_error,
        Severity::Warning => pal.editor_warning,
        Severity::Info | Severity::Hint => pal.editor_info,
    }
}

/// Отрезки текста от символа `from` до `to` по строкам раскладки (в координатах экрана).
fn segments(output: &TextEditOutput, from: usize, to: usize) -> Vec<Rect> {
    let g = &output.galley;
    let off = output.galley_pos.to_vec2();
    let a = g.pos_from_cursor(CCursor::new(from)).translate(off);
    let b = g.pos_from_cursor(CCursor::new(to.max(from))).translate(off);
    if (a.center().y - b.center().y).abs() < 1.0 {
        let right = if b.left() - a.left() < 4.0 { a.left() + 7.0 } else { b.left() };
        return vec![Rect::from_min_max(a.left_top(), Pos2::new(right, a.bottom()))];
    }
    let mut out = Vec::new();
    for row in &g.rows {
        let r = row.rect().translate(off);
        if r.bottom() <= a.top() || r.top() >= b.bottom() {
            continue;
        }
        let left = if r.center().y < a.bottom() { a.left() } else { r.left() };
        let right = if r.center().y > b.top() { b.left() } else { r.right() };
        if right > left {
            out.push(Rect::from_min_max(Pos2::new(left, r.top()), Pos2::new(right, r.bottom())));
        }
    }
    out
}

fn squiggle(painter: &egui::Painter, rect: Rect, color: Color32) {
    let y = rect.bottom() - 1.5;
    let mut pts = Vec::new();
    let mut x = rect.left();
    let mut up = true;
    while x <= rect.right() {
        pts.push(Pos2::new(x, if up { y - 1.5 } else { y + 0.5 }));
        x += 2.5;
        up = !up;
    }
    if pts.len() >= 2 {
        painter.add(Shape::line(pts, Stroke::new(1.1, color)));
    }
}

/// Подчёркивания ошибок, метки на полях и подсказка с текстом ошибки под указателем.
/// Возвращает текст ошибки под указателем.
pub fn paint_diagnostics(ui: &Ui, pal: &Palette, doc: &Document, lsp: &JavaLsp, output: &TextEditOutput, gutter: Rect, row_h: f32) -> Option<(Severity, String)> {
    let diags = lsp.diagnostics(&doc.path);
    if diags.is_empty() {
        return None;
    }
    let painter = ui.painter().with_clip_rect(ui.clip_rect());
    let pointer = ui.input(|i| i.pointer.hover_pos());
    let mut under: Option<(Severity, String)> = None;
    let mut marked_lines: Vec<(u32, Severity)> = Vec::new();
    // Сначала подсказки и предупреждения, ошибки — поверх.
    let mut sorted: Vec<_> = diags.iter().collect();
    sorted.sort_by(|a, b| b.severity.cmp(&a.severity));
    for d in sorted {
        let from = lsp::char_index(&doc.text, d.start.0, d.start.1);
        let to = lsp::char_index(&doc.text, d.end.0, d.end.1);
        let color = severity_color(pal, d.severity);
        for seg in segments(output, from, to) {
            if !ui.clip_rect().intersects(seg) {
                continue;
            }
            if d.severity <= Severity::Warning {
                squiggle(&painter, seg, color);
            } else {
                painter.line_segment([seg.left_bottom(), seg.right_bottom()], Stroke::new(1.0, color.gamma_multiply(0.6)));
            }
            if pointer.is_some_and(|p| seg.expand2(vec2(1.0, 2.0)).contains(p)) && under.as_ref().is_none_or(|(s, _)| d.severity < *s) {
                under = Some((d.severity, d.message.clone()));
            }
        }
        if d.severity <= Severity::Warning {
            marked_lines.push((d.start.0, d.severity));
        }
    }
    // Метка на полях: самая серьёзная проблема строки.
    marked_lines.sort();
    marked_lines.dedup_by(|a, b| a.0 == b.0);
    for (line, sev) in marked_lines {
        let y = output.galley_pos.y + line as f32 * row_h + row_h / 2.0;
        if y < ui.clip_rect().top() || y > ui.clip_rect().bottom() {
            continue;
        }
        painter.circle_filled(Pos2::new(gutter.left() + 7.0, y), 3.0, severity_color(pal, sev));
    }
    under
}

/// Описание символа под неподвижным указателем и текст ошибки, если указатель на ней.
pub fn hover(ui: &Ui, pal: &Palette, id: Id, doc: &Document, lsp: &mut JavaLsp, output: &TextEditOutput, diag: Option<(Severity, String)>) {
    let ctx = ui.ctx().clone();
    let Some(pointer) = ui.input(|i| i.pointer.hover_pos()) else { return };
    // `contains_pointer` учитывает слои: под диалогом или всплывающим окном редактор
    // указателя «не видит», и подсказка не просвечивает сквозь них.
    if !output.response.response.contains_pointer() || !ui.clip_rect().contains(pointer) || ui.input(|i| i.pointer.any_down()) {
        return;
    }
    let ci = output.galley.cursor_from_pos(pointer - output.galley_pos).index.0;
    let (ws, we) = word_range(&doc.text, ci);
    let still = ui.input(|i| i.pointer.time_since_last_movement()) >= HOVER_DELAY;
    let key = id.with("hover-requested");
    let requested: Option<(usize, usize)> = ctx.data(|d| d.get_temp(key));
    if still && we > ws && requested != Some((ws, we)) {
        lsp.request_hover(doc, ws);
        ctx.data_mut(|d| d.insert_temp(key, (ws, we)));
    }
    let info = lsp.hover.as_ref().filter(|h| h.doc == doc.path && h.at >= ws && h.at <= we && we > ws).map(|h| h.text.clone());
    if diag.is_none() && (info.is_none() || !still) {
        return;
    }
    egui::Tooltip::always_open(ctx.clone(), ui.layer_id(), id.with("hover-tip"), pointer).gap(12.0).show(|ui| {
        ui.set_max_width(520.0);
        if let Some((sev, msg)) = &diag {
            let icon = if *sev == Severity::Error { "✖" } else { "⚠" };
            ui.label(RichText::new(format!("{icon} {msg}")).color(severity_color(pal, *sev)));
        }
        if let Some(text) = info.filter(|_| still) {
            if diag.is_some() {
                ui.separator();
            }
            ui.label(RichText::new(text).font(FontId::monospace(12.0)).color(pal.text));
        }
    });
}

/// Ключевые слова Java: к ним не перейти, и при Ctrl они не подчёркиваются.
const JAVA_KEYWORDS: &[&str] = &[
    "abstract", "assert", "boolean", "break", "byte", "case", "catch", "char", "class", "const", "continue", "default", "do", "double", "else",
    "enum", "extends", "false", "final", "finally", "float", "for", "goto", "if", "implements", "import", "instanceof", "int", "interface",
    "long", "native", "new", "non", "null", "package", "permits", "private", "protected", "public", "record", "return", "sealed", "short",
    "static", "strictfp", "super", "switch", "synchronized", "this", "throw", "throws", "transient", "true", "try", "var", "void", "volatile",
    "while", "yield",
];

/// Стоит ли символ `at` в коде, а не в строке, символьном литерале или комментарии.
fn in_code(text: &str, at: usize) -> bool {
    #[derive(PartialEq)]
    enum St {
        Code,
        Line,
        Block,
        Str,
        Char,
        TextBlock,
    }
    let chars: Vec<char> = text.chars().take(at).collect();
    let mut st = St::Code;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        let triple = c == '"' && next == Some('"') && chars.get(i + 2) == Some(&'"');
        match st {
            St::Code if c == '/' && next == Some('/') => st = St::Line,
            St::Code if c == '/' && next == Some('*') => {
                st = St::Block;
                i += 1;
            }
            St::Code if triple => {
                st = St::TextBlock;
                i += 2;
            }
            St::Code if c == '"' => st = St::Str,
            St::Code if c == '\'' => st = St::Char,
            St::Line if c == '\n' => st = St::Code,
            St::Block if c == '*' && next == Some('/') => {
                st = St::Code;
                i += 1;
            }
            St::Str | St::Char | St::TextBlock if c == '\\' => i += 1,
            St::Str if c == '"' || c == '\n' => st = St::Code,
            St::Char if c == '\'' || c == '\n' => st = St::Code,
            St::TextBlock if triple => {
                st = St::Code;
                i += 2;
            }
            _ => {}
        }
        i += 1;
    }
    st == St::Code
}

/// Слово под символом `at`, к определению которого можно перейти: имя, а не
/// ключевое слово, число, строка или комментарий.
fn navigable_word(text: &str, at: usize) -> Option<(usize, usize)> {
    let (ws, we) = word_range(text, at);
    if we == ws {
        return None;
    }
    let word: String = text.chars().skip(ws).take(we - ws).collect();
    if word.starts_with(|c: char| c.is_ascii_digit()) || JAVA_KEYWORDS.contains(&word.as_str()) || !in_code(text, ws) {
        return None;
    }
    Some((ws, we))
}

/// Ctrl+щелчок по слову — перейти к определению. Позиция берётся из точки щелчка,
/// а не из курсора поля ввода: с зажатым Ctrl поле может обработать щелчок по-своему.
/// Пока Ctrl зажат, слово под указателем подчёркивается пунктиром, как ссылка.
pub fn ctrl_click(ui: &Ui, pal: &Palette, doc: &Document, lsp: &mut JavaLsp, output: &TextEditOutput) {
    let rect = output.response.response.rect.intersect(ui.clip_rect());
    // Под диалогом или всплывающим окном редактор не реагирует ни на наведение, ни на щелчок.
    if !output.response.response.contains_pointer() {
        return;
    }
    let (ctrl, hover) = ui.input(|i| (i.modifiers.command, i.pointer.hover_pos()));
    if let Some(p) = hover.filter(|p| ctrl && rect.contains(*p)) {
        let at = output.galley.cursor_from_pos(p - output.galley_pos).index.0;
        if let Some((ws, we)) = navigable_word(&doc.text, at) {
            let segs = segments(output, ws, we);
            // Указатель может стоять за концом строки — тогда ближайшее слово не под ним.
            if segs.iter().any(|s| s.expand2(vec2(0.0, 1.0)).contains(p)) {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                let painter = ui.painter().with_clip_rect(ui.clip_rect());
                for s in segs {
                    let y = s.bottom() - 1.0;
                    painter.extend(Shape::dashed_line(&[Pos2::new(s.left(), y), Pos2::new(s.right(), y)], Stroke::new(1.0, pal.editor_link), 3.0, 2.0));
                }
            }
        }
    }
    // Модификаторы берём из самого события нажатия: Ctrl мог отпуститься в том же кадре.
    let click = ui.input(|i| {
        i.events.iter().find_map(|e| match e {
            egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers } if modifiers.command && rect.contains(*pos) => Some(*pos),
            _ => None,
        })
    });
    if let Some(p) = click {
        let at = output.galley.cursor_from_pos(p - output.galley_pos).index.0;
        log::debug!("definition requested at char {at} in {}", doc.path.display());
        lsp.request_definition(doc, at);
    }
}

fn kind_glyph(pal: &Palette, kind: u32) -> (&'static str, Color32) {
    match kind {
        2..=4 => ("ƒ", pal.completion_method),
        5 | 6 | 10 => ("◆", pal.completion_field),
        7 | 22 => ("C", pal.completion_class),
        8 => ("I", pal.completion_interface),
        13 => ("E", pal.completion_enum),
        20 | 21 => ("◇", pal.completion_enum),
        9 | 14 => (if kind == 9 { "▣" } else { "k" }, pal.completion_keyword),
        25 => ("T", pal.completion_class),
        _ => ("·", pal.text_faint),
    }
}

/// Был ли список автодополнения этого редактора на экране в прошлом кадре.
pub fn popup_was_open(ctx: &egui::Context, id: Id) -> bool {
    ctx.data(|d| d.get_temp::<bool>(id.with("completion-open"))).unwrap_or(false)
}

/// Список автодополнения под началом слова.
#[allow(clippy::too_many_arguments)]
pub fn completion_popup(ui: &Ui, pal: &Palette, id: Id, doc: &mut Document, lsp: &mut JavaLsp, output: &TextEditOutput, has_focus: bool, now: f64) {
    let ctx = ui.ctx().clone();
    let open_key = id.with("completion-open");
    let scrolled_key = id.with("completion-scrolled");
    let was_open = ctx.data(|d| d.get_temp::<bool>(open_key)).unwrap_or(false);
    ctx.data_mut(|d| {
        d.insert_temp(open_key, false);
        // Список только что открылся: прокрутка от прошлого списка не в счёт.
        if !was_open {
            d.remove::<usize>(scrolled_key);
        }
    });
    if !has_focus {
        return;
    }
    let Some(c) = lsp.completion.as_ref().filter(|c| c.doc == doc.path) else { return };
    let Some(cursor) = output.cursor_range.map(|r| r.primary.index.0) else { return };
    let list = filtered(c, &typed_part(doc, c, cursor));
    if list.is_empty() && !c.loading {
        return;
    }
    ctx.data_mut(|d| d.insert_temp(open_key, true));
    let anchor = output.galley.pos_from_cursor(CCursor::new(c.anchor.min(doc.text.chars().count()))).translate(output.galley_pos.to_vec2());
    let selected = c.selected;
    // К выбранному пункту прокручиваем, только когда выбор сменился (стрелками или
    // фильтром) или список только что открылся. Если делать это каждый кадр,
    // прокрутка колёсиком тут же откатывается обратно.
    let scroll_to_selected = ctx.data(|d| d.get_temp::<usize>(scrolled_key)) != Some(selected);
    let mut scrolled = false;
    let loading = c.loading;
    let items: Vec<(usize, CompletionItem)> = list.iter().take(300).map(|&i| (i, c.items[i].clone())).collect();
    let mut clicked: Option<usize> = None;
    egui::Area::new(id.with("completion"))
        .order(Order::Foreground)
        .fixed_pos(anchor.left_bottom() + vec2(-6.0, 2.0))
        .show(&ctx, |ui| {
            Frame::new()
                .fill(pal.popup_bg)
                .stroke(Stroke::new(1.0, pal.popup_border))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(Margin::same(4))
                .shadow(egui::Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(80) })
                .show(ui, |ui| {
                    ui.set_width(440.0);
                    if items.is_empty() {
                        ui.label(RichText::new(format!("  {}", t!("code-completion-loading"))).color(pal.text_faint));
                        return;
                    }
                    // Минимальная высота по числу строк: в первом кадре список был пуст,
                    // и без неё область запомнила бы крошечный размер.
                    let rows = items.len().min(VISIBLE_ITEMS) as f32;
                    egui::ScrollArea::vertical()
                        .max_height(ITEM_H * VISIBLE_ITEMS as f32)
                        .min_scrolled_height(ITEM_H * rows)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for (i, item) in &items {
                            let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ITEM_H), Sense::click());
                            let is_sel = *i == selected;
                            if is_sel {
                                ui.painter().rect_filled(rect, CornerRadius::same(5), pal.completion_selected);
                                if scroll_to_selected {
                                    resp.scroll_to_me(None);
                                    scrolled = true;
                                }
                            } else if resp.hovered() {
                                ui.painter().rect_filled(rect, CornerRadius::same(5), pal.popup_hover);
                            }
                            let (glyph, color) = kind_glyph(pal, item.kind);
                            let cy = rect.center().y;
                            ui.painter().text(Pos2::new(rect.left() + 12.0, cy), Align2::CENTER_CENTER, glyph, FontId::monospace(12.0), color);
                            let painter = ui.painter().with_clip_rect(rect);
                            let label = painter.text(Pos2::new(rect.left() + 26.0, cy), Align2::LEFT_CENTER, &item.label, FontId::monospace(12.5), pal.completion_text);
                            // Подпись справа — только если помещается целиком и не налезает на имя.
                            if let Some(detail) = &item.detail {
                                let g = painter.layout_no_wrap(detail.clone(), FontId::proportional(11.5), pal.completion_detail);
                                if label.right() + 16.0 + g.size().x < rect.right() - 8.0 {
                                    painter.galley(Pos2::new(rect.right() - 8.0 - g.size().x, cy - g.size().y / 2.0), g, pal.completion_detail);
                                }
                            }
                            if resp.clicked() {
                                clicked = Some(*i);
                            }
                        }
                    });
                    if loading {
                        ui.label(RichText::new("…").color(pal.text_faint));
                    }
                });
        });
    if scrolled {
        ctx.data_mut(|d| d.insert_temp(scrolled_key, selected));
    }
    if let Some(i) = clicked {
        accept_item(&ctx, id, doc, lsp, i, now);
        ctx.memory_mut(|m| m.request_focus(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_around_cursor() {
        let text = "item.getDesc";
        assert_eq!(word_start(text, 12), 5);
        assert_eq!(word_range(text, 2), (0, 4));
        assert_eq!(word_start("a + b", 3), 3);
    }

    #[test]
    fn replace_by_char_indices() {
        let mut s = String::from("пре.fo бар");
        replace_chars(&mut s, 4, 6, "foo()");
        assert_eq!(s, "пре.foo() бар");
    }

    #[test]
    fn ctrl_underlines_only_names_in_code() {
        let src = "public class Ruby extends Block { // Item\n  String s = \"Item\"; /* Block */ int n = 42; char c = '\"'; Foo f;\n  String t = \"\"\"\n   Bar\n  \"\"\"; Baz b; }";
        let word_at = |needle: &str, nth: usize| {
            let byte = src.match_indices(needle).nth(nth).unwrap().0;
            let at = src[..byte].chars().count() + 1;
            navigable_word(src, at).map(|(a, b)| src.chars().skip(a).take(b - a).collect::<String>())
        };
        assert_eq!(word_at("Ruby", 0).as_deref(), Some("Ruby"));
        assert_eq!(word_at("Block", 0).as_deref(), Some("Block"));
        assert_eq!(word_at("public", 0), None, "ключевое слово");
        assert_eq!(word_at("Item", 0), None, "в однострочном комментарии");
        assert_eq!(word_at("Item", 1), None, "в строке");
        assert_eq!(word_at("Block", 1), None, "в блочном комментарии");
        assert_eq!(word_at("42", 0), None, "число");
        assert_eq!(word_at("Foo", 0).as_deref(), Some("Foo"), "кавычка в символьном литерале не открывает строку");
        assert_eq!(word_at("Bar", 0), None, "в текстовом блоке");
        assert_eq!(word_at("Baz", 0).as_deref(), Some("Baz"));
    }
}
