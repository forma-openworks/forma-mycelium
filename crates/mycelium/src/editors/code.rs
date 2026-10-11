//! Редактор кода: файлы проекта с подсветкой, номерами строк и автосохранением, как в IDEA,
//! и исходники библиотек из jar только для чтения. Подсказки jdtls — в `code_assist.rs`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use eframe::egui::{
    self, Align, Align2, CornerRadius, FontId, Frame, Galley, Id, Key, Margin, Modifiers, RichText, Sense, TextEdit, Ui,
    text::{CCursor, CCursorRange},
    vec2,
};
use forma_i18n::t;

use super::code_assist;
use crate::app::Cmd;
use crate::lsp::{JavaLsp, LspStatus};
use forma_ui::theme::Palette;

/// Файлы больше этого не открываем как текст.
const MAX_SIZE: u64 = 4 * 1024 * 1024;
/// Через сколько секунд после правки файл сохраняется сам.
const AUTOSAVE_DELAY: f64 = 1.0;
/// Отступ, который ставит Tab и автоотступ после `{`.
const INDENT: &str = "    ";

/// Открытый файл. Несколько вкладок одного файла делят один документ.
pub struct Document {
    pub path: PathBuf,
    pub text: String,
    pub dirty: bool,
    /// Файл был с переводами строк Windows — так и сохраним.
    crlf: bool,
    last_edit: f64,
    modified: Option<SystemTime>,
    pub lang: &'static str,
    /// Файл не открылся как текст.
    pub error: Option<String>,
    /// Перейти к строке (с единицы) на следующем кадре.
    pub jump: Option<u32>,
    /// Строка и столбец курсора (с единицы).
    pub cursor: (usize, usize),
    /// Высота строки на прошлом кадре — для подсветки текущей строки до раскладки текста.
    row_h: f32,
    /// Файл из jar (исходник Minecraft, ресурс библиотеки): только для чтения.
    jar: Option<(PathBuf, String)>,
    /// Растёт при каждом изменении текста — по нему языковой сервер узнаёт о правках.
    pub version: i32,
}

impl Document {
    fn load(path: &Path) -> Self {
        let mut doc = Document {
            path: path.to_path_buf(),
            text: String::new(),
            dirty: false,
            crlf: false,
            last_edit: 0.0,
            modified: None,
            lang: lang_for(path),
            error: None,
            jump: None,
            cursor: (1, 1),
            row_h: 0.0,
            jar: None,
            version: 0,
        };
        doc.read();
        doc
    }

    fn load_jar(jar: &Path, entry: &str) -> Self {
        let mut doc = Document { jar: Some((jar.to_path_buf(), entry.to_owned())), ..Document::load_empty(&jar_key(jar, entry)) };
        doc.read();
        doc
    }

    fn load_empty(path: &Path) -> Self {
        Document {
            path: path.to_path_buf(),
            text: String::new(),
            dirty: false,
            crlf: false,
            last_edit: 0.0,
            modified: None,
            lang: lang_for(path),
            error: None,
            jump: None,
            cursor: (1, 1),
            row_h: 0.0,
            jar: None,
            version: 0,
        }
    }

    /// Только для чтения: файлы из jar не правятся.
    pub fn readonly(&self) -> bool {
        self.jar.is_some()
    }

    fn read(&mut self) {
        if let Some((jar, entry)) = &self.jar {
            match mycelium_project::libraries::read_entry(jar, entry) {
                Ok(bytes) => self.set_bytes(bytes),
                Err(err) => self.error = Some(err.to_string()),
            }
            return;
        }
        let meta = std::fs::metadata(&self.path);
        self.modified = meta.as_ref().ok().and_then(|m| m.modified().ok());
        if meta.as_ref().is_ok_and(|m| m.len() > MAX_SIZE) {
            self.error = Some(t!("code-too-large"));
            return;
        }
        match std::fs::read(&self.path) {
            Ok(bytes) => self.set_bytes(bytes),
            Err(err) => self.error = Some(err.to_string()),
        }
    }

    fn set_bytes(&mut self, bytes: Vec<u8>) {
        if bytes.iter().take(8192).any(|b| *b == 0) {
            self.error = Some(t!("code-binary"));
            return;
        }
        match String::from_utf8(bytes) {
            Ok(text) => {
                self.crlf = text.contains("\r\n");
                self.text = if self.crlf { text.replace("\r\n", "\n") } else { text };
                self.error = None;
                self.dirty = false;
                self.version += 1;
            }
            Err(_) => self.error = Some(t!("code-not-utf8")),
        }
    }

    pub fn save(&mut self) -> Result<(), String> {
        if !self.dirty || self.error.is_some() || self.readonly() {
            return Ok(());
        }
        let text = if self.crlf { self.text.replace('\n', "\r\n") } else { self.text.clone() };
        std::fs::write(&self.path, text).map_err(|e| format!("{}: {e}", self.path.display()))?;
        self.dirty = false;
        self.modified = std::fs::metadata(&self.path).ok().and_then(|m| m.modified().ok());
        Ok(())
    }

    pub(crate) fn mark_edited(&mut self, now: f64) {
        self.dirty = true;
        self.last_edit = now;
        self.version += 1;
    }
}

/// Все открытые документы.
#[derive(Default)]
pub struct Documents {
    docs: HashMap<PathBuf, Document>,
}

/// Ключ документа из jar: путь jar + путь внутри (настоящих файлов «внутри» jar не бывает).
pub fn jar_key(jar: &Path, entry: &str) -> PathBuf {
    jar.join(entry)
}

impl Documents {
    pub fn open(&mut self, path: &Path) -> &mut Document {
        self.docs.entry(path.to_path_buf()).or_insert_with(|| Document::load(path))
    }

    /// Файл из jar — только для чтения.
    pub fn open_jar(&mut self, jar: &Path, entry: &str) -> &mut Document {
        self.docs.entry(jar_key(jar, entry)).or_insert_with(|| Document::load_jar(jar, entry))
    }

    pub fn get_mut(&mut self, path: &Path) -> Option<&mut Document> {
        self.docs.get_mut(path)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Document> {
        self.docs.values()
    }

    pub fn is_dirty(&self, path: &Path) -> bool {
        self.docs.get(path).is_some_and(|d| d.dirty)
    }

    /// Сохраняет все изменённые файлы. Возвращает ошибки.
    pub fn save_all(&mut self) -> Vec<String> {
        self.docs.values_mut().filter_map(|d| d.save().err()).collect()
    }

    /// Сохраняет файлы, которые не трогали дольше задержки. Возвращает ошибки и ждёт ли кто-то ещё.
    pub fn autosave(&mut self, now: f64) -> (Vec<String>, bool) {
        let mut errors = Vec::new();
        let mut waiting = false;
        for d in self.docs.values_mut().filter(|d| d.dirty) {
            if now - d.last_edit >= AUTOSAVE_DELAY {
                if let Err(e) = d.save() {
                    errors.push(e);
                    // Не повторяем ошибку на каждом кадре.
                    d.last_edit = now + 30.0;
                }
            } else {
                waiting = true;
            }
        }
        (errors, waiting)
    }

    /// Перечитывает файлы, изменённые снаружи (если в редакторе их не трогали).
    pub fn reload_changed(&mut self) {
        for d in self.docs.values_mut().filter(|d| !d.dirty && !d.readonly()) {
            let modified = std::fs::metadata(&d.path).ok().and_then(|m| m.modified().ok());
            if modified.is_some() && modified != d.modified {
                d.read();
            }
        }
    }

    /// Забывает документы, для которых не осталось вкладок (несохранённые сначала сохраняются).
    pub fn retain_open(&mut self, open: &HashSet<PathBuf>) {
        self.docs.retain(|path, d| {
            if open.contains(path) {
                return true;
            }
            if let Err(e) = d.save() {
                log::warn!("{e}");
                return true;
            }
            false
        });
    }

    pub fn clear(&mut self) {
        self.save_all();
        self.docs.clear();
    }
}

/// Язык подсветки по расширению файла.
pub fn lang_for(path: &Path) -> &'static str {
    let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "java" => "java",
        "kt" | "kts" => "kt",
        "json" | "mcmeta" | "jsonc" => "json",
        "toml" => "toml",
        "gradle" | "groovy" => "groovy",
        "properties" | "cfg" | "lang" => "properties",
        "xml" => "xml",
        "md" => "markdown",
        "yml" | "yaml" => "yaml",
        "fsh" | "vsh" | "glsl" | "frag" | "vert" => "glsl",
        "js" => "js",
        "bat" | "cmd" => "bat",
        "sh" => "bash",
        _ => "",
    }
}

/// Индекс символа (не байта) начала строки `line` (с единицы).
fn line_start_char(text: &str, line: u32) -> usize {
    if line <= 1 {
        return 0;
    }
    let mut seen = 1;
    for (i, c) in text.chars().enumerate() {
        if c == '\n' {
            seen += 1;
            if seen == line {
                return i + 1;
            }
        }
    }
    text.chars().count()
}

fn char_to_byte(s: &str, ci: usize) -> usize {
    s.char_indices().nth(ci).map(|(b, _)| b).unwrap_or(s.len())
}

/// Строка и столбец (с единицы) символа с индексом `ci`.
fn line_col(text: &str, ci: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for c in text.chars().take(ci) {
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// Заменяет выделение в редакторе на `insert` и ставит курсор после вставки.
fn replace_selection(text: &mut String, range: CCursorRange, insert: &str) -> usize {
    let (a, b) = (range.primary.index.0.min(range.secondary.index.0), range.primary.index.0.max(range.secondary.index.0));
    let (ba, bb) = (char_to_byte(text, a), char_to_byte(text, b));
    text.replace_range(ba..bb, insert);
    a + insert.chars().count()
}

/// Отступ строки, в которой стоит курсор, и нужен ли дополнительный (после `{`, `(`, `[`).
fn indent_for_newline(text: &str, ci: usize) -> String {
    let byte = char_to_byte(text, ci);
    let line_start = text[..byte].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line = &text[line_start..byte];
    let indent: String = line.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
    let opens = line.trim_end().ends_with(['{', '(', '[']);
    if opens { format!("{indent}{INDENT}") } else { indent }
}

/// Редактор файла. `rel` — путь для заголовка. `lsp` — подсказки Java (если проект их ведёт).
#[allow(clippy::too_many_arguments)]
pub fn ui(ui: &mut Ui, doc: &mut Document, rel: &str, pal: &Palette, tab_id: u64, fonts: &forma_ui::fonts::Fonts, now: f64, lsp: Option<&mut JavaLsp>, keys: super::Keys, cmds: &mut Vec<Cmd>) {
    header(ui, doc, rel, pal);
    // Подсказки — только для Java-файлов проекта (не для исходников из jar).
    let java = doc.lang == "java" && !doc.readonly();
    let assist: Option<&mut JavaLsp> = if java { lsp } else { None };
    if let Some(l) = assist.as_deref() {
        lsp_banner(ui, pal, l, cmds);
    }
    let mut assist = assist.filter(|l| l.is_ready());
    if let Some(err) = &doc.error {
        ui.add_space(30.0);
        ui.vertical_centered(|ui| {
            super::big_icon(ui, "file", 30.0, pal.text_faint);
            ui.label(RichText::new(t!("code-cannot-open")).size(15.0).color(pal.heading));
            ui.label(RichText::new(err).size(12.5).color(pal.text_muted));
        });
        return;
    }

    let id = Id::new(("mycelium-code", tab_id));
    let ctx = ui.ctx().clone();
    // egui снимает фокус по Escape ещё до нас. Если был открыт список автодополнения,
    // Escape только закрывает его, а фокус остаётся в редакторе, как в IDEA.
    if code_assist::popup_was_open(&ctx, id) && ctx.input(|i| i.key_pressed(Key::Escape)) {
        if let Some(l) = assist.as_deref_mut() {
            l.completion = None;
        }
        ctx.memory_mut(|m| m.request_focus(id));
    }
    let has_focus = ctx.memory(|m| m.has_focus(id));
    let size = fonts.editor.size;
    let line_height = fonts.editor.row_height();

    // Tab — четыре пробела, Enter — с отступом предыдущей строки.
    let readonly = doc.readonly();
    if let Some(l) = assist.as_deref_mut() {
        code_assist::apply_server_edits(&ctx, id, doc, l, now);
    }
    // Список автодополнения забирает стрелки, Enter и Tab раньше поля ввода.
    let handled = has_focus && assist.as_deref_mut().is_some_and(|l| code_assist::handle_keys(&ctx, id, doc, l, keys, now));
    if has_focus && !readonly && !handled {
        let tab = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab));
        let enter = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
        if tab || enter {
            if let Some(mut state) = TextEdit::load_state(&ctx, id) {
                if let Some(range) = state.cursor.char_range() {
                    let insert = if tab { INDENT.to_owned() } else { format!("\n{}", indent_for_newline(&doc.text, range.primary.index.0.min(range.secondary.index.0))) };
                    let at = replace_selection(&mut doc.text, range, &insert);
                    state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(at))));
                    state.store(&ctx, id);
                    doc.mark_edited(now);
                }
            }
        }
    }

    let lines = doc.text.split('\n').count();
    let digits = lines.to_string().len().max(3) as f32;
    let gutter_w = digits * size * 0.62 + 22.0;
    let lang = doc.lang;
    let mut layouter = |ui: &Ui, buf: &dyn egui::TextBuffer, _wrap: f32| {
        let mut job = crate::highlight::job(ui.ctx(), buf.as_str(), lang, size, line_height);
        job.wrap.max_width = f32::INFINITY;
        let galley = ui.fonts_mut(|f| f.layout_job(job));
        center_rows(ui.ctx(), id, galley)
    };

    let avail = ui.available_size();
    // Ctrl+колёсико над редактором меняет размер его шрифта.
    if fonts.ctrl_wheel && ui.rect_contains_pointer(ui.max_rect()) {
        let step = super::ctrl_wheel_font_step(ui, id);
        if step != 0.0 {
            cmds.push(Cmd::FontZoom { console: false, step });
        }
    }
    ui.painter().rect_filled(ui.available_rect_before_wrap(), CornerRadius::ZERO, pal.editor_bg);
    egui::ScrollArea::both().id_salt(("mycelium-code-scroll", tab_id)).auto_shrink(false).show(ui, |ui| {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let (gutter, _) = ui.allocate_exact_size(vec2(gutter_w, 0.0), Sense::hover());
            if pal.editor_gutter != pal.editor_bg {
                let g = egui::Rect::from_x_y_ranges(gutter.x_range(), ui.clip_rect().y_range());
                ui.painter().rect_filled(g, CornerRadius::ZERO, pal.editor_gutter);
            }
            // Выделение и курсор — свои у редактора.
            ui.visuals_mut().selection.bg_fill = pal.editor_selection;
            ui.visuals_mut().selection.stroke.color = pal.editor_selection_text;
            ui.visuals_mut().text_cursor.stroke.color = pal.editor_cursor;
            // Подсветка строки с курсором (по высоте строки с прошлого кадра).
            let top = ui.cursor().top();
            if doc.row_h > 0.0 && has_focus {
                let y = top + (doc.cursor.0 as f32 - 1.0) * doc.row_h;
                let r = egui::Rect::from_min_size(egui::pos2(gutter.left(), y), vec2(ui.clip_rect().right() - gutter.left(), doc.row_h));
                ui.painter().rect_filled(r, CornerRadius::ZERO, pal.editor_current_line);
            }
            let mut show = |buffer: &mut dyn egui::TextBuffer, ui: &mut Ui| {
                TextEdit::multiline(buffer)
                    .id(id)
                    .code_editor()
                    .frame(Frame::NONE)
                    .margin(Margin::symmetric(6, 0))
                    .desired_width(f32::INFINITY)
                    .min_size(vec2((avail.x - gutter_w).max(100.0), avail.y.max(100.0)))
                    .layouter(&mut layouter)
                    .show(ui)
            };
            // Файл из jar можно выделять и копировать, но не править.
            let output = if readonly { show(&mut doc.text.as_str(), ui) } else { show(&mut doc.text, ui) };
            let changed = output.response.response.changed() && !readonly;
            if changed {
                doc.mark_edited(now);
            }
            if let Some(l) = assist.as_deref_mut() {
                code_assist::after_input(doc, l, &output, changed);
            }
            if let Some(r) = output.cursor_range {
                doc.cursor = line_col(&doc.text, r.primary.index.0);
            }
            let rows = output.galley.rows.len().max(1);
            let row_h = output.galley.rows.first().map(|r| r.rect().height()).unwrap_or(line_height);
            doc.row_h = row_h;

            // Номера строк.
            let painter = ui.painter().with_clip_rect(ui.clip_rect());
            let visible = ui.clip_rect();
            let font = FontId::new(size * 0.9, forma_ui::fonts::editor(false, false));
            for i in 0..rows {
                let y = output.galley_pos.y + i as f32 * row_h;
                if y + row_h < visible.top() || y > visible.bottom() {
                    continue;
                }
                let current = has_focus && i + 1 == doc.cursor.0;
                let color = if current { pal.editor_line_number_current } else { pal.editor_line_number };
                painter.text(egui::pos2(gutter.right() - 10.0, y + row_h / 2.0), Align2::RIGHT_CENTER, (i + 1).to_string(), font.clone(), color);
            }

            // Ошибки компилятора, описания, переход к определению и автодополнение.
            if let Some(l) = assist.as_deref_mut() {
                let diag = code_assist::paint_diagnostics(ui, pal, doc, l, &output, gutter, row_h);
                code_assist::hover(ui, pal, id, doc, l, &output, diag);
                code_assist::ctrl_click(ui, pal, doc, l, &output);
                code_assist::completion_popup(ui, pal, id, doc, l, &output, has_focus, now);
            }

            // Переход к строке (из исключений, ошибок компиляции).
            if let Some(line) = doc.jump.take() {
                let ci = line_start_char(&doc.text, line);
                let mut state = output.state.clone();
                state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(ci))));
                state.store(&ctx, id);
                ctx.memory_mut(|m| m.request_focus(id));
                let rect = output.galley.pos_from_cursor(CCursor::new(ci)).translate(output.galley_pos.to_vec2());
                ui.scroll_to_rect(rect.expand2(vec2(0.0, row_h * 4.0)), Some(Align::Center));
                doc.cursor = (line as usize, 1);
            }
        });
    });
}

/// Ставит текст каждой строки посередине её высоты.
///
/// Когда высота строки из настроек больше высоты шрифта, egui кладёт глифы к верху
/// строки, и код «прилипает» к её верхней границе, а номер строки (он по центру) кажется
/// съехавшим. Здесь глифы каждой строки сдвигаются вниз на половину лишней высоты.
/// Прямоугольники строк не меняются, поэтому курсор, выделение и щелчки остаются точными.
/// Сдвинутая раскладка запоминается и пересчитывается, только когда egui выдал новую.
fn center_rows(ctx: &egui::Context, id: Id, galley: Arc<Galley>) -> Arc<Galley> {
    let key = id.with("centered-galley");
    if let Some((src, out)) = ctx.data(|d| d.get_temp::<(Arc<Galley>, Arc<Galley>)>(key))
        && Arc::ptr_eq(&src, &galley)
    {
        return out;
    }
    let ppp = ctx.pixels_per_point();
    let mut out = galley.clone();
    let g = Arc::make_mut(&mut out);
    let mut lowest = 0.0_f32;
    for placed in &mut g.rows {
        let Some(first) = placed.row.glyphs.first() else { continue };
        let dy = ((first.line_height - first.font_height) * 0.5 * ppp).round() / ppp;
        if dy <= 0.0 {
            continue;
        }
        let row = Arc::make_mut(&mut placed.row);
        for glyph in &mut row.glyphs {
            glyph.pos.y += dy;
        }
        for v in &mut row.visuals.mesh.vertices {
            v.pos.y += dy;
        }
        row.visuals.mesh_bounds = row.visuals.mesh_bounds.translate(vec2(0.0, dy));
        lowest = lowest.max(dy);
    }
    g.mesh_bounds.max.y += lowest;
    ctx.data_mut(|d| d.insert_temp(key, (galley, out.clone())));
    out
}

/// Полоска под заголовком, пока подсказки Java не готовы: установка jdtls, запуск, импорт, ошибка.
fn lsp_banner(ui: &mut Ui, pal: &Palette, lsp: &JavaLsp, cmds: &mut Vec<Cmd>) {
    let mb = |b: u64| b as f64 / 1_048_576.0;
    let (text, color, action): (String, egui::Color32, Option<(String, Cmd)>) = match &lsp.status {
        LspStatus::Ready => return,
        LspStatus::NotInstalled => (t!("lsp-not-installed"), pal.text_muted, Some((t!("lsp-install"), Cmd::InstallPlugin(crate::plugins::JDTLS.into())))),
        LspStatus::Installing(forma_plugins::Progress::Downloading { done, total: Some(total) }) => {
            (t!("lsp-downloading-of", done = format!("{:.1}", mb(*done)), total = format!("{:.1}", mb(*total))), pal.accent, None)
        }
        LspStatus::Installing(forma_plugins::Progress::Downloading { done, total: None }) => (t!("lsp-downloading", done = format!("{:.1}", mb(*done))), pal.accent, None),
        LspStatus::Installing(_) => (t!("lsp-extracting"), pal.accent, None),
        LspStatus::NoJdk(need) => (t!("lsp-no-jdk", version = *need), pal.warning, None),
        LspStatus::Starting => (t!("lsp-starting"), pal.text_muted, None),
        // Импорт идёт в фоне, подсказки уже частично работают — тихая строка.
        LspStatus::Importing(step) => {
            let step = lsp.progress().map(str::to_owned).unwrap_or_else(|| step.clone());
            (if step.is_empty() { t!("lsp-importing") } else { format!("{} · {step}", t!("lsp-importing")) }, pal.text_faint, None)
        }
        LspStatus::Failed(e) => (t!("lsp-failed", error = e.clone()), pal.danger, Some((t!("lsp-restart"), Cmd::RestartJdtls))),
    };
    egui::Frame::new().fill(color.gamma_multiply(0.08)).inner_margin(Margin::symmetric(10, 3)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        // Текст переносится по ширине области, кнопка — следом за ним.
        ui.horizontal_wrapped(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(14.0, 16.0), Sense::hover());
            forma_ui::icons::paint(ui.painter(), r, "jdk", color);
            ui.add(egui::Label::new(RichText::new(text).size(12.0).color(color)).wrap());
            if let Some((label, cmd)) = action {
                if ui.small_button(label).clicked() {
                    cmds.push(cmd);
                }
            }
        });
    });
}

/// Строка над редактором: путь файла, позиция курсора, язык, отметка о несохранённом.
/// В узком редакторе сведения справа теряют лишнее (переводы строк, язык, «только чтение»),
/// а у пути обрезается начало — имя файла видно всегда.
fn header(ui: &mut Ui, doc: &Document, rel: &str, pal: &Palette) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::hover());
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    painter.rect_filled(rect, CornerRadius::ZERO, pal.editor_header);
    let font = FontId::proportional(12.0);
    let path = rel.replace('\\', " › ").replace('/', " › ");
    let pos = format!("{}:{}", doc.cursor.0, doc.cursor.1);
    let mut parts: Vec<String> = vec![pos.clone()];
    if !doc.lang.is_empty() {
        parts.push(lang_name(doc.lang).to_owned());
    }
    if doc.crlf {
        parts.push("CRLF".to_owned());
    }
    // Варианты от полного к краткому: пути должно остаться хотя бы 120 точек.
    let variants = [parts.join("  ·  "), parts.iter().filter(|p| *p != "CRLF").cloned().collect::<Vec<_>>().join("  ·  "), pos.clone()];
    let room = rect.width() - 20.0 - if doc.dirty { 16.0 } else { 0.0 };
    let info = variants
        .iter()
        .map(|v| painter.layout_no_wrap(v.clone(), font.clone(), pal.text_faint))
        .find(|g| room - g.size().x >= 120.0)
        .unwrap_or_else(|| painter.layout_no_wrap(String::new(), font.clone(), pal.text_faint));
    let info_w = info.size().x;
    painter.galley(egui::pos2(rect.right() - 10.0 - info_w, rect.center().y - info.size().y / 2.0), info, pal.text_faint);
    let mut path_right = rect.right() - 10.0 - info_w - 12.0;
    if doc.dirty {
        let x = rect.right() - 10.0 - info_w - if info_w > 0.0 { 10.0 } else { 4.0 };
        painter.circle_filled(egui::pos2(x, rect.center().y), 3.5, pal.accent);
        path_right = x - 10.0;
    }
    // Файл только для чтения (исходник из jar) — замок перед путём.
    let mut path_left = rect.left() + 10.0;
    if doc.readonly() {
        forma_ui::icons::paint(&painter, egui::Rect::from_center_size(egui::pos2(path_left + 6.0, rect.center().y), vec2(13.0, 13.0)), "lock", pal.text_faint);
        path_left += 18.0;
    }
    let galley = forma_ui::widgets::elide_start(ui, &path, font, pal.editor_header_text, path_right - path_left);
    let elided = galley.size().x + 0.5 < painter.layout_no_wrap(path.clone(), FontId::proportional(12.0), pal.editor_header_text).size().x;
    painter.galley(egui::pos2(path_left, rect.center().y - galley.size().y / 2.0), galley, pal.editor_header_text);
    match (elided, doc.readonly()) {
        (true, true) => {
            resp.on_hover_text(format!("{path}\n{}", t!("code-readonly")));
        }
        (true, false) => {
            resp.on_hover_text(path);
        }
        (false, true) => {
            resp.on_hover_text(t!("code-readonly"));
        }
        (false, false) => {}
    }
}

fn lang_name(lang: &str) -> &'static str {
    match lang {
        "java" => "Java",
        "kt" => "Kotlin",
        "json" => "JSON",
        "toml" => "TOML",
        "groovy" => "Groovy",
        "properties" => "Properties",
        "xml" => "XML",
        "markdown" => "Markdown",
        "yaml" => "YAML",
        "glsl" => "GLSL",
        "js" => "JavaScript",
        "bat" => "Batch",
        "bash" => "Shell",
        _ => "Text",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_positions() {
        let text = "a\nbc\nдвa";
        assert_eq!(line_start_char(text, 1), 0);
        assert_eq!(line_start_char(text, 2), 2);
        assert_eq!(line_start_char(text, 3), 5);
        assert_eq!(line_start_char(text, 99), 8);
        assert_eq!(line_col(text, 6), (3, 2));
    }

    #[test]
    fn auto_indent_after_brace() {
        let text = "class A {\n    void b() {";
        let end = text.chars().count();
        assert_eq!(indent_for_newline(text, end), "        ");
        assert_eq!(indent_for_newline("    x();", 8), "    ");
    }

    #[test]
    fn crlf_roundtrip_and_autosave() {
        let dir = std::env::temp_dir().join(format!("mycelium-code-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("A.java");
        std::fs::write(&path, "class A {\r\n}\r\n").unwrap();
        let mut docs = Documents::default();
        let doc = docs.open(&path);
        assert_eq!(doc.text, "class A {\n}\n");
        assert_eq!(doc.lang, "java");
        doc.text.push_str("// x\n");
        doc.mark_edited(10.0);
        assert_eq!(docs.autosave(10.5).1, true, "ещё ждём");
        assert!(docs.is_dirty(&path));
        docs.autosave(11.5);
        assert!(!docs.is_dirty(&path));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "class A {\r\n}\r\n// x\r\n");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
