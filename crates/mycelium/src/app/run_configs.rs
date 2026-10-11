//! Окно «Конфигурации запуска»: найденные в проекте и свои. Найденные можно переименовать,
//! поменять им задачу и аргументы, скрыть и вернуть как было; свои — добавить, скопировать
//! и удалить. Всё сохраняется в `.mycelium/runs.json`.

use eframe::egui::{self, CornerRadius, CursorIcon, FontId, Margin, RichText, Sense, Stroke, TextEdit, Ui, vec2};
use forma_i18n::t;

use crate::run::{RunConfig, RunKind, RunsFile};
use forma_ui::theme::Palette;

struct Row {
    config: RunConfig,
    /// Как конфигурация выглядела при обнаружении (у своих — нет).
    original: Option<RunConfig>,
    hidden: bool,
}

impl Row {
    fn edited(&self) -> bool {
        self.original.as_ref().is_some_and(|o| !same(o, &self.config))
    }
}

/// Совпадают ли настройки (без признака «найдена»).
fn same(a: &RunConfig, b: &RunConfig) -> bool {
    a.name.as_deref().unwrap_or("").trim() == b.name.as_deref().unwrap_or("").trim()
        && a.task.trim() == b.task.trim()
        && a.args.trim() == b.args.trim()
        && a.kind == b.kind
        && a.module == b.module
}

pub enum Outcome {
    Done,
    Cancel,
}

pub struct RunConfigsEditor {
    rows: Vec<Row>,
    selected: usize,
    /// Задачи Gradle проекта — подсказки для поля «Задача».
    tasks: Vec<String>,
}

impl RunConfigsEditor {
    pub fn new(detected: Vec<RunConfig>, file: &RunsFile, tasks: Vec<String>, select: Option<&str>) -> Self {
        let mut rows: Vec<Row> = detected
            .into_iter()
            .map(|d| {
                let config = match file.overrides.get(&d.id) {
                    Some(o) => RunConfig { id: d.id.clone(), detected: true, ..o.clone() },
                    None => d.clone(),
                };
                Row { hidden: file.hidden.contains(&d.id), config, original: Some(d) }
            })
            .collect();
        rows.extend(file.custom.iter().map(|c| Row { config: RunConfig { detected: false, ..c.clone() }, original: None, hidden: false }));
        let selected = select.and_then(|id| rows.iter().position(|r| r.config.id == id)).unwrap_or(0);
        Self { rows, selected, tasks }
    }

    /// Что сохранить в `runs.json`.
    pub fn to_file(&self) -> RunsFile {
        let mut file = RunsFile { version: 1, ..RunsFile::default() };
        for r in &self.rows {
            let mut c = r.config.clone();
            c.name = c.name.filter(|n| !n.trim().is_empty());
            c.task = c.task.trim().to_owned();
            c.args = c.args.trim().to_owned();
            match &r.original {
                Some(_) => {
                    if r.hidden {
                        file.hidden.push(c.id.clone());
                    }
                    if r.edited() {
                        file.overrides.insert(c.id.clone(), c);
                    }
                }
                None if !c.task.is_empty() => file.custom.push(c),
                None => {}
            }
        }
        file
    }

    /// Выбранная конфигурация (чтобы после «Готово» она стала текущей).
    pub fn selected_id(&self) -> Option<String> {
        self.rows.get(self.selected).filter(|r| !r.hidden).map(|r| r.config.id.clone())
    }

    fn new_id(&self) -> String {
        (1..).map(|n| format!("custom-{n}")).find(|id| !self.rows.iter().any(|r| &r.config.id == id)).expect("свободный номер найдётся")
    }

    pub fn ui(&mut self, ui: &mut Ui, pal: &Palette) -> Option<Outcome> {
        let mut outcome = None;
        ui.label(RichText::new(t!("runcfg-title")).size(18.0).family(forma_ui::fonts::semibold()).color(pal.heading));
        ui.add_space(10.0);
        // В широком окне список слева от полей, в узком — над ними.
        if ui.available_width() >= 560.0 {
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(vec2(240.0, 380.0), egui::Layout::top_down(egui::Align::Min), |ui| self.list(ui, pal, 232.0, 320.0));
                ui.add_space(14.0);
                ui.allocate_ui_with_layout(vec2(ui.available_width(), 380.0), egui::Layout::top_down(egui::Align::Min), |ui| self.form(ui, pal));
            });
        } else {
            let width = ui.available_width();
            self.list(ui, pal, width - 8.0, 150.0);
            ui.add_space(10.0);
            self.form(ui, pal);
        }
        ui.add_space(14.0);
        let mut done = false;
        let mut cancel = false;
        forma_ui::widgets::bar(
            ui,
            egui::Id::new("mycelium-runcfg-footer"),
            0.0,
            |ui| {
                ui.label(RichText::new(t!("runcfg-saved-in")).size(11.5).color(pal.text_faint));
            },
            |ui| {
                done = forma_ui::settings_window::primary_button(ui, pal, &t!("action-done")).clicked();
                cancel = ui.add(egui::Button::new(t!("action-cancel")).min_size(vec2(84.0, 30.0))).clicked();
            },
        );
        if done {
            outcome = Some(Outcome::Done);
        } else if cancel {
            outcome = Some(Outcome::Cancel);
        }
        outcome
    }

    /// Список конфигураций шириной `width` и высотой не больше `height`, кнопки под ним.
    fn list(&mut self, ui: &mut Ui, pal: &Palette, width: f32, height: f32) {
        egui::Frame::new().fill(pal.bg_deep).corner_radius(8).inner_margin(Margin::same(4)).stroke(Stroke::new(1.0, pal.border)).show(ui, |ui| {
            ui.set_width(width);
            egui::ScrollArea::vertical().max_height(height).auto_shrink([false, false]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                for (i, r) in self.rows.iter().enumerate() {
                    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
                    let selected = i == self.selected;
                    if selected {
                        ui.painter().rect_filled(rect, CornerRadius::same(6), pal.list_selected);
                    } else if resp.hovered() {
                        ui.painter().rect_filled(rect, CornerRadius::same(6), pal.list_hover);
                    }
                    let color = if r.hidden { pal.text_faint } else if selected { pal.heading } else { pal.text };
                    let painter = ui.painter().with_clip_rect(rect.shrink(2.0));
                    painter.text(rect.left_center() + vec2(14.0, 0.0), egui::Align2::CENTER_CENTER, r.config.kind.icon(), FontId::proportional(12.5), color);
                    let mut label = r.config.label();
                    if r.edited() {
                        label.push_str(" *");
                    }
                    let tag = if r.hidden {
                        Some(t!("runcfg-hidden"))
                    } else if r.original.is_some() {
                        Some("Gradle".to_owned())
                    } else {
                        None
                    };
                    // Название обрезается, чтобы метка «Gradle» или «скрыта» осталась видна.
                    let tag = tag.map(|t| painter.layout_no_wrap(t, FontId::proportional(10.5), pal.text_faint));
                    let room = rect.width() - 28.0 - 6.0 - tag.as_ref().map_or(0.0, |g| g.size().x + 6.0);
                    let name = forma_ui::widgets::elide(ui, label, FontId::proportional(13.0), color, room);
                    let name_w = name.size().x;
                    painter.galley(egui::pos2(rect.left() + 28.0, rect.center().y - name.size().y / 2.0), name, color);
                    if let Some(g) = tag {
                        painter.galley(egui::pos2(rect.left() + 28.0 + name_w + 6.0, rect.center().y - g.size().y / 2.0), g, pal.text_faint);
                    }
                    if resp.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(r.config.command_line()).clicked() {
                        self.selected = i;
                    }
                }
            });
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button(format!("＋ {}", t!("runcfg-new"))).clicked() {
                let id = self.new_id();
                let config = RunConfig { id, name: Some(t!("runcfg-new-name")), task: String::new(), args: String::new(), kind: RunKind::Other, module: None, detected: false };
                self.rows.push(Row { config, original: None, hidden: false });
                self.selected = self.rows.len() - 1;
            }
            let Some(row) = self.rows.get(self.selected) else { return };
            if ui.button("⧉").on_hover_text(t!("runcfg-duplicate")).clicked() {
                let id = self.new_id();
                let mut config = RunConfig { id, detected: false, ..row.config.clone() };
                config.name = Some(t!("runcfg-copy-name", name = row.config.label()));
                self.rows.insert(self.selected + 1, Row { config, original: None, hidden: false });
                self.selected += 1;
                return;
            }
            if row.original.is_some() {
                let hidden = row.hidden;
                let (glyph, hint) = if hidden { ("👁", t!("runcfg-show")) } else { ("🚫", t!("runcfg-hide")) };
                if ui.button(glyph).on_hover_text(hint).clicked() {
                    self.rows[self.selected].hidden = !hidden;
                }
            } else if ui.button("🗑").on_hover_text(t!("runcfg-delete")).clicked() {
                self.rows.remove(self.selected);
                self.selected = self.selected.min(self.rows.len().saturating_sub(1));
            }
        });
    }

    /// Поля выбранной конфигурации.
    fn form(&mut self, ui: &mut Ui, pal: &Palette) {
        let tasks = &self.tasks;
        let Some(row) = self.rows.get_mut(self.selected) else {
            ui.label(RichText::new(t!("runcfg-empty")).color(pal.text_faint));
            return;
        };
        let field = |ui: &mut Ui, text: String| {
            ui.add_space(6.0);
            ui.label(RichText::new(text).size(12.5).color(pal.text_muted));
            ui.add_space(2.0);
        };
        let note = if row.original.is_some() { t!("runcfg-detected-note") } else { t!("runcfg-custom-note") };
        ui.label(RichText::new(note).size(12.0).color(pal.text_faint));

        field(ui, t!("runcfg-name"));
        let auto = RunConfig { name: None, ..row.config.clone() }.label();
        let mut name = row.config.name.clone().unwrap_or_default();
        if ui.add(TextEdit::singleline(&mut name).hint_text(auto).desired_width(f32::INFINITY).margin(Margin::symmetric(8, 5))).changed() {
            row.config.name = Some(name);
        }

        field(ui, t!("runcfg-kind"));
        egui::ComboBox::from_id_salt("runcfg-kind")
            .selected_text(format!("{}  {}", row.config.kind.icon(), row.config.kind.label()))
            .width(ui.available_width().min(220.0))
            .show_ui(ui, |ui| {
                for k in RunKind::ALL {
                    ui.selectable_value(&mut row.config.kind, k, format!("{}  {}", k.icon(), k.label()));
                }
            });
        ui.label(RichText::new(t!("runcfg-kind-hint")).size(11.5).color(pal.text_faint));

        field(ui, t!("runcfg-task"));
        ui.add(TextEdit::singleline(&mut row.config.task).hint_text("runClient").font(FontId::monospace(13.0)).desired_width(f32::INFINITY).margin(Margin::symmetric(8, 5)));
        // Подсказки: задачи проекта, начинающиеся с набранного или содержащие его.
        let typed = row.config.task.split_whitespace().last().unwrap_or("").to_lowercase();
        let matches: Vec<&String> = if typed.is_empty() {
            tasks.iter().filter(|t| t.rsplit(':').next().is_some_and(|n| n.starts_with("run"))).take(8).collect()
        } else {
            tasks.iter().filter(|t| t.to_lowercase().contains(&typed) && t.to_lowercase() != typed).take(8).collect()
        };
        if !matches.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                for m in matches {
                    if ui.add(egui::Button::new(RichText::new(m).font(FontId::monospace(11.5))).small()).clicked() {
                        // Заменяем последнее слово: так можно набрать несколько задач подряд.
                        let mut words: Vec<&str> = row.config.task.split_whitespace().collect();
                        if !typed.is_empty() {
                            words.pop();
                        }
                        words.push(m);
                        row.config.task = words.join(" ");
                        if row.original.is_none() && row.config.kind == RunKind::Other {
                            row.config.kind = RunKind::of_task(m);
                        }
                    }
                }
            });
        }

        field(ui, t!("runcfg-args"));
        ui.add(
            TextEdit::singleline(&mut row.config.args)
                .hint_text("--offline --stacktrace -Pkey=value")
                .font(FontId::monospace(13.0))
                .desired_width(f32::INFINITY)
                .margin(Margin::symmetric(8, 5)),
        );

        ui.add_space(10.0);
        egui::Frame::new().fill(pal.bg_deep).corner_radius(6).inner_margin(Margin::symmetric(10, 6)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(row.config.command_line()).font(FontId::monospace(12.0)).color(pal.text_muted));
        });
        if row.edited() {
            ui.add_space(6.0);
            if ui.link(t!("runcfg-reset")).clicked() {
                if let Some(o) = &row.original {
                    row.config = o.clone();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detected() -> Vec<RunConfig> {
        let mk = |id: &str, task: &str, kind| RunConfig { id: id.into(), name: None, task: task.into(), args: String::new(), kind, module: None, detected: true };
        vec![mk("client", "runClient", RunKind::Client), mk("server", "runServer", RunKind::Server), mk("build", "build", RunKind::Build)]
    }

    #[test]
    fn untouched_editor_writes_empty_file() {
        let e = RunConfigsEditor::new(detected(), &RunsFile::default(), Vec::new(), None);
        assert_eq!(e.to_file(), RunsFile { version: 1, ..RunsFile::default() });
    }

    #[test]
    fn edits_hides_and_custom_configs_roundtrip() {
        let mut e = RunConfigsEditor::new(detected(), &RunsFile::default(), Vec::new(), Some("server"));
        assert_eq!(e.selected, 1);
        e.rows[0].config.args = "--offline".into();
        e.rows[1].hidden = true;
        let id = e.new_id();
        e.rows.push(Row {
            config: RunConfig { id, name: Some("Проверка".into()), task: " check ".into(), args: String::new(), kind: RunKind::Other, module: None, detected: false },
            original: None,
            hidden: false,
        });
        let file = e.to_file();
        assert_eq!(file.overrides["client"].args, "--offline");
        assert_eq!(file.hidden, vec!["server".to_owned()]);
        assert_eq!(file.custom[0].task, "check");
        assert_eq!(file.custom[0].id, "custom-1");

        // Открыли снова — всё на месте.
        let again = RunConfigsEditor::new(detected(), &file, Vec::new(), None);
        assert!(again.rows[0].edited());
        assert!(again.rows[1].hidden);
        assert_eq!(again.rows[3].config.label(), "Проверка");
        assert_eq!(again.to_file(), file);
    }
}
