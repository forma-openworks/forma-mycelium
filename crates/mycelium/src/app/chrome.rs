//! Оформление главного окна: верхняя строка с меню, рабочими пространствами и запуском,
//! строка состояния, экран приветствия, диалоги и уведомления.

use std::path::PathBuf;

use eframe::egui::{self, Align, Align2, Color32, CornerRadius, CursorIcon, FontId, Frame, Id, Key, Layout, Margin, Rect, RichText, Sense, Stroke, TextEdit, Ui, vec2};
use forma_i18n::t;

use super::run_configs::Outcome;
use super::{APP_TITLE, Cmd, Modal, MyceliumApp};
use crate::actions;
use crate::editors::EditorKind;
use crate::icon::Logo;
use forma_ui::theme::{self, Origin, Palette};
use forma_ui::widgets;
use crate::workspace::Builtin;

/// Иконка нужного размера в пикселях экрана — так она остаётся чёткой.
pub(super) fn icon_texture(ctx: &egui::Context, cache: &mut Option<(u32, egui::TextureHandle)>, logo: Logo, points: f32, name: &str) -> egui::TextureHandle {
    let px = (points * ctx.pixels_per_point()).round().max(16.0) as u32;
    if let Some((size, tex)) = cache {
        if *size == px {
            return tex.clone();
        }
    }
    let img = egui::ColorImage::from_rgba_unmultiplied([px as usize, px as usize], &crate::icon::rgba(logo, px));
    let tex = ctx.load_texture(name, img, egui::TextureOptions::LINEAR);
    *cache = Some((px, tex.clone()));
    tex
}

impl MyceliumApp {
    pub(super) fn top_bar(&mut self, ui: &mut Ui) {
        let pal = self.pal.clone();
        let mark = icon_texture(ui.ctx(), &mut self.mark, Logo::Simple, 20.0, "mycelium-mark");
        // Справа отступа нет: там вплотную к краю окна стоят кнопки окна (см. `caption`).
        let frame = Frame::new().fill(pal.menubar_bg).inner_margin(Margin { left: 8, right: 0, top: 4, bottom: 4 });
        let bar = egui::Panel::top("mycelium-top").frame(frame).show(ui, |ui| {
            // Вся строка — заголовок окна: за пустое место окно перетаскивают.
            let inner = ui.max_rect();
            let whole = Rect::from_min_max(inner.min - vec2(8.0, 4.0), inner.max + vec2(0.0, 4.0));
            super::caption::drag_area(ui, whole, Id::new("mycelium-caption"));
            let w = &mut ui.visuals_mut().widgets;
            w.inactive.fg_stroke.color = pal.menubar_text;
            w.hovered.fg_stroke.color = pal.menubar_text;
            w.hovered.weak_bg_fill = pal.menubar_hover;
            w.open.weak_bg_fill = pal.menubar_hover;
            egui::MenuBar::new().style(theme::menu_bar_style).config(theme::menu_config()).ui(ui, |ui| {
                // Место в строке делится по важности: значок и меню, кнопки запуска, текущее
                // рабочее пространство, список конфигураций, остальные пространства. Что не
                // поместилось — уходит в меню: пространства в «»», а меню программы в одно «☰».
                let total = ui.available_width() - super::caption::BUTTONS_W - 8.0;
                let gap = ui.spacing().item_spacing.x;
                let menus_w: f32 = menu_titles().iter().map(|t| widgets::button_width(ui, t, 0.0) + gap).sum();
                let burger_w = forma_ui::icons::SIZE + 2.0 * ui.spacing().button_padding.x + gap;
                let run_w = if self.project.is_some() { self.run_buttons_width(ui) } else { 0.0 };
                let active_title = self.layout.workspaces[self.layout.active].title();
                // Текущее пространство и кнопки «+» и «»» рядом с ним.
                let tabs_min = workspace_tab_width(ui, &active_title) + 60.0;
                let logo_w = 24.0 + gap;
                let compact = logo_w + menus_w + 18.0 + tabs_min + run_w > total;
                let left_w = logo_w + if compact { burger_w + 6.0 } else { menus_w + 18.0 } + tabs_min;
                let combo = Some((total - left_w - run_w - 8.0).min(190.0)).filter(|w| *w >= 100.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(super::caption::BUTTONS_W + 8.0);
                    if self.project.is_some() {
                        ui.horizontal(|ui| {
                            crate::editors::console::run_bar(ui, &pal, &self.console, &self.configs, &mut self.view.run_config, combo, &mut self.cmds);
                        });
                    }
                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                        ui.add(egui::Image::new(egui::load::SizedTexture::new(mark.id(), vec2(20.0, 20.0))));
                        ui.add_space(4.0);
                        self.menus(ui, &pal, compact);
                        ui.add_space(if compact { 6.0 } else { 18.0 });
                        self.workspace_tabs(ui, &pal);
                    });
                });
            });
        });
        self.caption_h = bar.response.rect.height();
    }

    /// Ширина кнопок запуска в верхней строке (без списка конфигураций).
    fn run_buttons_width(&self, ui: &Ui) -> f32 {
        let gap = ui.spacing().item_spacing.x;
        let running = self.selected_config().is_some_and(|c| self.console.is_running_config(&c.id));
        let button = ui.spacing().interact_size.y + gap;
        let mut w = button;
        if running {
            w += button;
        }
        if self.console.running().count() > 1 {
            w += button;
        }
        w
    }

    /// Меню программы. `compact` — все меню свёрнуты в одно «☰» (узкое окно).
    fn menus(&mut self, ui: &mut Ui, pal: &Palette, compact: bool) {
        if compact {
            widgets::top_menu_icon(ui, "menu", &t!("menu-compact"), false, |ui| {
                ui.menu_button(t!("menu-file"), |ui| self.file_menu(ui, pal));
                ui.menu_button(t!("menu-view"), |ui| self.view_menu(ui));
                ui.menu_button(t!("menu-run"), |ui| self.run_menu(ui, pal));
                ui.menu_button(t!("menu-window"), |ui| self.window_menu(ui));
                ui.menu_button(t!("menu-help"), |ui| self.help_menu(ui));
            });
            return;
        }
        widgets::top_menu(ui, &t!("menu-file"), false, |ui| self.file_menu(ui, pal));
        // «Вид» не закрывается от переключателей, чтобы можно было поменять сразу несколько.
        widgets::top_menu(ui, &t!("menu-view"), true, |ui| self.view_menu(ui));
        widgets::top_menu(ui, &t!("menu-run"), false, |ui| self.run_menu(ui, pal));
        widgets::top_menu(ui, &t!("menu-window"), false, |ui| self.window_menu(ui));
        widgets::top_menu(ui, &t!("menu-help"), false, |ui| self.help_menu(ui));
    }

    fn file_menu(&mut self, ui: &mut Ui, pal: &Palette) {
        if ui.add(item(t!("action-open-project"), self.keys(actions::OPEN_PROJECT))).clicked() {
            self.cmds.push(Cmd::OpenProjectDialog);
        }
        ui.menu_button(t!("menu-recent-projects"), |ui| {
            let recent: Vec<PathBuf> = self.settings.recent.projects.iter().filter(|p| p.is_dir()).cloned().collect();
            if recent.is_empty() {
                ui.label(RichText::new(t!("menu-recent-empty")).color(pal.text_faint));
            }
            for path in recent {
                let name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                if ui.button(name).on_hover_text(path.display().to_string()).clicked() {
                    self.cmds.push(Cmd::OpenProject(path));
                }
            }
        });
        if ui.add_enabled(self.project.is_some(), item(t!("action-close-project"), self.keys(actions::CLOSE_PROJECT))).clicked() {
            self.cmds.push(Cmd::CloseProject);
        }
        ui.separator();
        if ui.add_enabled(self.project.is_some(), item(t!("action-save-all"), self.keys(actions::SAVE_ALL))).clicked() {
            self.cmds.push(Cmd::SaveAll);
        }
        ui.separator();
        if ui.add(item(t!("action-settings"), self.keys(actions::SETTINGS))).clicked() {
            self.cmds.push(Cmd::Settings);
        }
        ui.menu_button(t!("menu-manage-settings"), |ui| {
            if ui.add(item(t!("action-export-settings"), self.keys(actions::EXPORT_SETTINGS))).clicked() {
                self.cmds.push(Cmd::ExportSettings);
            }
            if ui.add(item(t!("action-import-settings"), self.keys(actions::IMPORT_SETTINGS))).clicked() {
                self.cmds.push(Cmd::ImportSettings);
            }
        });
        ui.separator();
        let quit = self.keys(actions::QUIT);
        if ui.add(item(t!("action-quit"), if quit.is_empty() { "Alt+F4".to_owned() } else { quit })).clicked() {
            self.cmds.push(Cmd::Quit);
        }
    }

    /// Меню «Вид»: тема и масштаб. Ползунков здесь нет — они живут в настройках.
    fn view_menu(&mut self, ui: &mut Ui) {
        ui.menu_button(t!("settings-theme"), |ui| {
            let current = if self.settings.appearance.theme.is_empty() { crate::colors::DEFAULT_THEME.to_owned() } else { self.settings.appearance.theme.clone() };
            for th in self.themes.list() {
                // Встроенные темы помечены справа, как сочетание клавиш у пункта.
                let tag = if th.origin == Origin::Builtin { t!("theme-default") } else { String::new() };
                if ui.add(egui::Button::new(th.name.as_str()).shortcut_text(tag).selected(current == th.id)).clicked() {
                    self.settings.appearance.theme = th.id.clone();
                }
            }
            ui.separator();
            if ui.add(item(t!("action-quick-theme"), self.keys(actions::QUICK_THEME))).clicked() {
                self.cmds.push(Cmd::QuickSwitchTheme);
                ui.close();
            }
        });
        ui.separator();
        if ui.add(item(t!("action-zoom-in"), self.keys(actions::ZOOM_IN))).clicked() {
            self.cmds.push(Cmd::ZoomIn);
        }
        if ui.add(item(t!("action-zoom-out"), self.keys(actions::ZOOM_OUT))).clicked() {
            self.cmds.push(Cmd::ZoomOut);
        }
        if ui.add(item(t!("action-zoom-reset"), self.keys(actions::ZOOM_RESET))).clicked() {
            self.cmds.push(Cmd::ZoomReset);
        }
    }

    fn run_menu(&mut self, ui: &mut Ui, pal: &Palette) {
        if self.configs.is_empty() {
            ui.label(RichText::new(t!("run-no-configs")).color(pal.text_faint));
        }
        for c in &self.configs {
            // Идущий запуск помечен точкой; повторный выбор перезапускает его.
            let running = self.console.is_running_config(&c.id);
            let text = if running { format!("{}  ●", c.label()) } else { c.label() };
            if forma_ui::icons::item(ui, c.kind.icon(), text, false).on_hover_text(c.command_line()).clicked() {
                self.cmds.push(Cmd::RunConfig(c.id.clone()));
            }
        }
        ui.separator();
        let selected_running = self.selected_config().is_some_and(|c| self.console.is_running_config(&c.id));
        if ui.add_enabled(selected_running, item(t!("run-stop"), self.keys(actions::STOP))).clicked() {
            self.cmds.push(Cmd::Stop);
        }
        if ui.add_enabled(self.console.is_running(), item(t!("run-stop-all"), self.keys(actions::STOP_ALL))).clicked() {
            self.cmds.push(Cmd::StopAll);
        }
        ui.separator();
        let edit = icon_item(ui, "edit", t!("run-edit-configs"), self.keys(actions::EDIT_RUN_CONFIGS));
        if ui.add_enabled(self.project.is_some(), edit).clicked() {
            self.cmds.push(Cmd::EditRunConfigs);
        }
    }

    fn window_menu(&mut self, ui: &mut Ui) {
        ui.menu_button(t!("menu-open-editor"), |ui| {
            for kind in EditorKind::ALL {
                if forma_ui::icons::item(ui, kind.icon(), kind.title(), false).clicked() {
                    self.cmds.push(Cmd::OpenEditor(kind));
                }
            }
        });
        ui.separator();
        if ui.add(item(t!("dock-maximize"), self.keys(actions::MAXIMIZE))).clicked() {
            self.cmds.push(Cmd::ToggleMaximize);
        }
        if ui.add(item(t!("dock-detach"), self.keys(actions::DETACH))).clicked() {
            self.cmds.push(Cmd::DetachFocused);
        }
        if ui.add_enabled(!self.layout.dock.windows.is_empty(), item(t!("action-return-windows"), self.keys(actions::RETURN_WINDOWS))).clicked() {
            self.cmds.push(Cmd::ReturnAllWindows);
        }
        ui.separator();
        ui.menu_button(t!("menu-new-workspace"), |ui| self.new_workspace_items(ui));
        let builtin = self.layout.workspaces[self.layout.active].builtin.is_some();
        if ui.add_enabled(builtin, item(t!("action-reset-workspace"), String::new())).clicked() {
            self.cmds.push(Cmd::ResetWorkspace(self.layout.active));
        }
        if ui.add(item(t!("action-reset-all-workspaces"), String::new())).clicked() {
            self.cmds.push(Cmd::ResetAllWorkspaces);
        }
        ui.separator();
        if ui.add(item(t!("layout-save-default"), String::new())).on_hover_text(t!("layout-save-default-hint")).clicked() {
            self.cmds.push(Cmd::SaveDefaultLayout);
        }
        if ui.add(item(t!("layout-apply-default"), String::new())).clicked() {
            self.cmds.push(Cmd::ApplyDefaultLayout);
        }
        if ui.add(item(t!("layout-reset-default"), String::new())).clicked() {
            self.cmds.push(Cmd::ResetDefaultLayout);
        }
    }

    fn help_menu(&mut self, ui: &mut Ui) {
        if ui.add(item(t!("action-find-action"), self.keys(actions::FIND_ACTION))).clicked() {
            self.cmds.push(Cmd::FindAction);
        }
        ui.separator();
        if ui.add(item(t!("action-about"), self.keys(actions::ABOUT))).clicked() {
            self.cmds.push(Cmd::About);
        }
    }

    fn new_workspace_items(&mut self, ui: &mut Ui) {
        if ui.button(t!("workspace-new-copy")).clicked() {
            self.cmds.push(Cmd::NewWorkspace(None));
        }
        ui.separator();
        for b in Builtin::ALL {
            if ui.button(b.title()).clicked() {
                self.cmds.push(Cmd::NewWorkspace(Some(b)));
            }
        }
    }

    /// Вкладки рабочих пространств в верхней строке, как в Blender. Те, что не поместились,
    /// открываются из меню «»»; текущее пространство видно всегда.
    fn workspace_tabs(&mut self, ui: &mut Ui, pal: &Palette) {
        let count = self.layout.workspaces.len();
        let active = self.layout.active;
        let titles: Vec<String> = self.layout.workspaces.iter().map(|w| w.title()).collect();
        let gap = ui.spacing().item_spacing.x;
        let widths: Vec<f32> = titles.iter().map(|t| workspace_tab_width(ui, t)).collect();
        let plus_w = 24.0 + gap;
        let more_w = 28.0 + gap;
        let avail = ui.available_width() - plus_w;
        let mut visible = vec![true; count];
        if widths.iter().map(|w| w + gap).sum::<f32>() > avail {
            // Сначала текущее, затем остальные по порядку, пока есть место.
            let room = avail - more_w;
            let mut used = widths[active] + gap;
            for (i, w) in widths.iter().enumerate() {
                visible[i] = i == active || used + w + gap <= room;
                if i != active && visible[i] {
                    used += w + gap;
                }
            }
        }
        // Текущее пространство в совсем узком окне сжимается до оставшегося места.
        let active_max = (avail - if visible.iter().all(|v| *v) { 0.0 } else { more_w }).max(40.0);
        for i in (0..count).filter(|i| visible[*i]) {
            let max_w = if i == active { active_max } else { f32::INFINITY };
            let resp = workspace_tab(ui, pal, Id::new(("mycelium-workspace", i)), &titles[i], i == active, max_w);
            if resp.double_clicked() {
                self.cmds.push(Cmd::RenameWorkspace(i));
            } else if resp.clicked() {
                self.cmds.push(Cmd::SwitchWorkspace(i));
            }
            let builtin = self.layout.workspaces[i].builtin.is_some();
            theme::context_menu(&resp, |ui| {
                if ui.button(t!("workspace-rename")).clicked() {
                    self.cmds.push(Cmd::RenameWorkspace(i));
                }
                if ui.button(t!("workspace-duplicate")).clicked() {
                    self.cmds.push(Cmd::DuplicateWorkspace(i));
                }
                if ui.add_enabled(builtin, egui::Button::new(t!("action-reset-workspace"))).clicked() {
                    self.cmds.push(Cmd::ResetWorkspace(i));
                }
                ui.separator();
                if ui.add_enabled(count > 1, egui::Button::new(t!("workspace-delete"))).clicked() {
                    self.cmds.push(Cmd::DeleteWorkspace(i));
                }
            });
        }
        let hidden: Vec<usize> = (0..count).filter(|i| !visible[*i]).collect();
        if !hidden.is_empty() {
            let more = forma_ui::icons::flat_button(ui, "overflow", pal.text_muted).on_hover_text(t!("workspace-more", count = hidden.len()));
            egui::Popup::menu(&more).style(theme::menu_style).show(|ui| {
                for i in hidden {
                    if ui.button(&titles[i]).clicked() {
                        self.cmds.push(Cmd::SwitchWorkspace(i));
                    }
                }
            });
        }
        let plus = forma_ui::icons::flat_button(ui, "add", pal.text_muted).on_hover_text(t!("menu-new-workspace"));
        egui::Popup::menu(&plus).style(theme::menu_style).show(|ui| self.new_workspace_items(ui));
    }

    pub(super) fn status_bar(&mut self, ui: &mut Ui) {
        let pal = self.pal.clone();
        egui::Panel::bottom("mycelium-status").frame(Frame::new().fill(pal.statusbar_bg).inner_margin(Margin::symmetric(12, 3))).show(ui, |ui| {
            let small = |s: String, c: Color32| RichText::new(s).size(12.0).color(c);
            // Справа — язык и видеокарта, слева — всё остальное. Сведения о видеокарте
            // уступают место левой части, а левая часть обрезается по краю правой.
            let left_id = Id::new("mycelium-status-left-width");
            let left_w: f32 = ui.data(|d| d.get_temp(left_id)).unwrap_or(0.0);
            let gpu_w = ui.painter().layout_no_wrap(self.gpu_info.clone(), FontId::proportional(12.0), pal.statusbar_text).size().x + 24.0;
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(small(forma_i18n::language().to_uppercase(), pal.statusbar_text)).on_hover_text(t!("settings-language"));
                    if left_w + gpu_w + 40.0 <= ui.available_width() {
                        ui.label(small("·".into(), pal.statusbar_text));
                        ui.label(small(self.gpu_info.clone(), pal.statusbar_text)).on_hover_text(t!("status-renderer"));
                    }
                    let rest = ui.available_rect_before_wrap();
                    let mut left = ui.new_child(egui::UiBuilder::new().max_rect(rest).layout(Layout::left_to_right(Align::Center)));
                    left.set_clip_rect(rest.shrink2(vec2(0.0, -4.0)).intersect(ui.clip_rect()));
                    self.status_left(&mut left, &pal);
                    let w = left.min_rect().width();
                    if (w - left_w).abs() > 0.5 {
                        ui.data_mut(|d| d.insert_temp(left_id, w));
                    }
                });
            });
        });
    }

    /// Левая часть строки состояния: проект, синхронизация, Java, запуски, проблемы.
    fn status_left(&mut self, ui: &mut Ui, pal: &Palette) {
        let small = |s: String, c: Color32| RichText::new(s).size(12.0).color(c);
        match &self.project {
            Some(p) => {
                ui.label(small("●".into(), pal.success));
                ui.label(small(p.name().to_owned(), pal.statusbar_text_strong)).on_hover_text(p.root.display().to_string());
                let info = &p.info;
                if info.is_mod() {
                    let loaders: Vec<&str> = info.loaders.iter().map(|l| l.name()).collect();
                    let mut text = loaders.join(" + ");
                    if let Some(mc) = &info.minecraft {
                        text.push_str(&format!(" · {mc}"));
                    }
                    ui.label(small(text, pal.statusbar_text));
                }
            }
            None => {
                ui.label(small(t!("status-no-project"), pal.statusbar_text));
            }
        }
        let item = crate::editors::problem_ui::icon_count;
        if self.sync.is_busy() {
            item(ui, "refresh", &format!("{}…", t!("status-syncing")), 12.0, pal.accent);
        }
        if let Some(lsp) = &self.lsp {
            let java = match &lsp.status {
                crate::lsp::LspStatus::Ready => match lsp.problem_counts() {
                    (0, 0) => item(ui, "jdk", "Java", 12.0, pal.statusbar_text),
                    (e, w) => {
                        // Значок Java и счётчики ошибок и предупреждений компилятора.
                        let r = item(ui, "jdk", "", 12.0, pal.statusbar_text);
                        let r = r.union(item(ui, "error", &e.to_string(), 12.0, if e > 0 { pal.danger } else { pal.statusbar_text }));
                        r.union(item(ui, "warning", &w.to_string(), 12.0, if w > 0 { pal.warning } else { pal.statusbar_text }))
                    }
                },
                crate::lsp::LspStatus::NotInstalled => item(ui, "jdk", &t!("status-lsp-off"), 12.0, pal.statusbar_text),
                crate::lsp::LspStatus::Failed(_) | crate::lsp::LspStatus::NoJdk(_) => item(ui, "jdk", &t!("status-lsp-error"), 12.0, pal.danger),
                _ => item(ui, "jdk", &format!("{}…", t!("status-lsp-busy")), 12.0, pal.accent),
            };
            java.on_hover_text(t!("status-lsp-hint"));
        }
        // Все идущие запуски: «▶ Клиент · 1:23 · Сервер · 0:45».
        let now = ui.input(|i| i.time);
        let running: Vec<String> = self
            .console
            .running()
            .map(|s| {
                let secs = match s.status {
                    crate::run::Status::Running { since } => (now - since).max(0.0) as u64,
                    _ => 0,
                };
                format!("{} · {}:{:02}", s.config.label(), secs / 60, secs % 60)
            })
            .collect();
        if !running.is_empty() {
            let resp = item(ui, "run", &running.join("  ·  "), 12.0, pal.accent);
            if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                self.cmds.push(Cmd::OpenEditor(EditorKind::Console));
            }
        }
        // Безобидный шум Minecraft в счётчик не входит.
        let problems: Vec<_> = self.console.problems.problems().iter().filter(|p| !p.sample.noise).collect();
        if !problems.is_empty() {
            let errors = problems.iter().filter(|p| p.sample.crash || p.sample.level >= mycelium_log::Level::Error || p.sample.throwable.is_some()).count();
            let warnings = problems.len() - errors;
            let resp = item(ui, "error", &errors.to_string(), 12.0, if errors > 0 { pal.danger } else { pal.statusbar_text });
            let resp = resp.union(item(ui, "warning", &warnings.to_string(), 12.0, if warnings > 0 { pal.warning } else { pal.statusbar_text }));
            if resp.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(t!("status-problems")).clicked() {
                self.cmds.push(Cmd::OpenEditor(EditorKind::Exceptions));
            }
        }
        let windows = self.layout.dock.windows.len();
        if windows > 0 {
            ui.label(small("·".into(), pal.statusbar_text));
            ui.label(small(t!("status-detached-windows", count = windows), pal.statusbar_text));
        }
    }

    pub(super) fn welcome_ui(&mut self, ui: &mut Ui) {
        let pal = self.pal.clone();
        let logo = icon_texture(ui.ctx(), &mut self.logo, Logo::Detailed, 104.0, "mycelium-logo");
        egui::CentralPanel::default().frame(Frame::new().fill(pal.welcome_bg)).show(ui, |ui| {
            // Строки меню здесь нет: заголовок окна — полоса вдоль верхнего края.
            let strip = Rect::from_min_size(ui.max_rect().min, vec2(ui.max_rect().width(), super::caption::WELCOME_H));
            super::caption::drag_area(ui, strip, Id::new("mycelium-caption-welcome"));
            // В низком окне экран приветствия компактнее (кнопки видны сразу) и прокручивается.
            let height = ui.available_height();
            let compact = height < 600.0;
            let (logo_size, title_size, gap) = if compact { (64.0, 30.0, 14.0) } else { (104.0, 38.0, 26.0) };
            egui::ScrollArea::vertical().id_salt("mycelium-welcome").auto_shrink(false).show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(if compact { 16.0 } else { (height * 0.14).max(24.0) });
                    ui.add(egui::Image::new(egui::load::SizedTexture::new(logo.id(), vec2(logo_size, logo_size))));
                    ui.add_space(if compact { 8.0 } else { 14.0 });
                    // «FORMA» вразрядку над названием, как подпись студии.
                    ui.label(RichText::new("F O R M A").size(13.0).family(forma_ui::fonts::semibold()).color(pal.welcome_brand));
                    ui.add_space(2.0);
                    ui.label(RichText::new("Mycelium").size(title_size).family(forma_ui::fonts::bold()).color(pal.welcome_title));
                    ui.add_space(6.0);
                    widgets::centered_text(ui, &t!("welcome-tagline"), 15.0, pal.text_muted, 460.0);
                    ui.add_space(gap);
                    let width = (ui.available_width() - 32.0).clamp(120.0, 300.0);
                    let ctx = ui.ctx().clone();
                    let big = |icon: &str, text: String| {
                        let img = forma_ui::icons::image(&ctx, icon, 18.0, pal.text);
                        egui::Button::new((img, RichText::new(text).size(15.5))).min_size(vec2(width, 40.0)).corner_radius(10).truncate()
                    };
                    if ui.add(big("open", t!("action-open-project")).fill(pal.accent_soft)).clicked() {
                        self.cmds.push(Cmd::OpenProjectDialog);
                    }
                    ui.add_space(6.0);
                    if ui.add(big("continue", t!("welcome-continue"))).clicked() {
                        self.cmds.push(Cmd::ContinueWithoutProject);
                    }
                    // Рекомендуемые плагины: ставятся одной кнопкой, пока их нет.
                    let catalog = crate::plugins::catalog();
                    let mut plugin_actions = Vec::new();
                    ui.add_space(gap);
                    forma_ui::plugins_page::welcome(ui, &pal, &self.plugins, &catalog, (ui.available_width() - 32.0).clamp(240.0, 460.0), &mut plugin_actions);
                    self.cmds.extend(plugin_actions.into_iter().map(super::settings_dialog::plugin_cmd));
                    let recent: Vec<PathBuf> = self.settings.recent.projects.iter().filter(|p| p.is_dir()).cloned().collect();
                    if !recent.is_empty() {
                        ui.add_space(gap * 0.5);
                        ui.label(RichText::new(t!("welcome-recent")).size(13.0).color(pal.text_faint));
                        ui.add_space(4.0);
                        for p in recent.into_iter().take(6) {
                            let name = p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                            let resp = ui.add(egui::Button::new(RichText::new(&name).size(14.5)).frame(false).truncate()).on_hover_text(p.display().to_string());
                            if resp.clicked() {
                                self.cmds.push(Cmd::OpenProject(p));
                            }
                        }
                    }
                    ui.add_space(gap);
                    let gear = forma_ui::icons::image(ui.ctx(), "settings", 15.0, pal.text_muted);
                    if ui.add(egui::Button::new((gear, RichText::new(t!("action-settings")).size(13.0).color(pal.text_muted))).frame(false)).clicked() {
                        self.cmds.push(Cmd::Settings);
                    }
                    ui.add_space(4.0);
                    ui.label(RichText::new(format!("{} · {}", t!("status-renderer"), self.gpu_info)).size(12.0).color(pal.text_faint));
                    ui.add_space(16.0);
                });
            });
        });
    }

    pub(super) fn modals(&mut self, ctx: &egui::Context) {
        let anim_id = Id::new("mycelium-modal-anim");
        // Диалог вынимаем на время отрисовки: внутри него меняются настройки приложения.
        let Some(mut modal) = self.modal.take() else {
            ctx.animate_bool_with_time(anim_id, false, 0.0);
            return;
        };
        // Диалог плавно проявляется вместе с затемнением фона.
        let t = ctx.animate_bool_with_time(anim_id, true, 0.16);
        let pal = self.pal.clone();
        let now = Self::now(ctx);
        let focus = std::mem::take(&mut self.modal_focus);
        let mut close = false;
        let margin = if matches!(modal, Modal::Settings(_)) { 0 } else { 22 };
        let frame = Frame::new()
            .fill(pal.modal_bg.gamma_multiply(t))
            .stroke(Stroke::new(1.0, pal.modal_border.gamma_multiply(t)))
            .corner_radius(CornerRadius::same(14))
            .inner_margin(Margin::same(margin));
        let mut rename_done: Option<(usize, String)> = None;
        // Диалог не выходит за окно: что не помещается по высоте — прокручивается.
        let max_h = (ctx.content_rect().height() - 2.0 * margin as f32 - 40.0).max(120.0);
        let resp = egui::Modal::new(Id::new("mycelium-modal"))
            .frame(frame)
            .backdrop_color(pal.modal_backdrop.gamma_multiply(t))
            .show(ctx, |ui| {
                ui.multiply_opacity(t);
                let title = |ui: &mut Ui, text: String| {
                    ui.label(RichText::new(text).size(18.0).family(forma_ui::fonts::semibold()).color(pal.heading));
                    ui.add_space(12.0);
                };
                let primary = |ui: &mut Ui, text: String| forma_ui::settings_window::primary_button(ui, &pal, &text).clicked();
                match &mut modal {
                    Modal::Settings(dialog) => close = self.settings_ui(ui, &pal, dialog),
                    Modal::FindAction(find) => close = self.find_action_ui(ui, &pal, find),
                    Modal::ThemeSwitcher(switcher) => close = self.theme_switcher_ui(ui, &pal, switcher),
                    Modal::Export(export) => close = egui::ScrollArea::vertical().max_height(max_h).show(ui, |ui| self.export_ui(ui, &pal, export, now)).inner,
                    Modal::Import(import) => close = egui::ScrollArea::vertical().max_height(max_h).show(ui, |ui| self.import_ui(ui, &pal, import)).inner,
                    Modal::About => {
                        ui.set_width(modal_width(ui.ctx(), 380.0));
                        let logo = icon_texture(ui.ctx(), &mut self.logo, Logo::Detailed, 104.0, "mycelium-logo");
                        egui::ScrollArea::vertical().max_height(max_h).show(ui, |ui| ui.vertical_centered(|ui| {
                            ui.add(egui::Image::new(egui::load::SizedTexture::new(logo.id(), vec2(72.0, 72.0))));
                            ui.add_space(8.0);
                            ui.label(RichText::new(APP_TITLE).size(20.0).family(forma_ui::fonts::bold()).color(pal.heading));
                            ui.label(RichText::new(t!("about-version", version = env!("CARGO_PKG_VERSION"))).size(12.5).color(pal.text_faint));
                            ui.add_space(8.0);
                            widgets::centered_text(ui, &t!("about-description"), 13.5, pal.text_muted, 340.0);
                            ui.add_space(8.0);
                            ui.label(RichText::new(format!("{} · {}", t!("status-renderer"), self.gpu_info)).size(12.0).color(pal.text_faint));
                            ui.label(RichText::new(t!("about-forma")).size(12.0).color(pal.accent));
                            ui.add_space(12.0);
                            if primary(ui, t!("action-close")) {
                                close = true;
                            }
                        }));
                    }
                    Modal::RunConfigs(editor) => {
                        ui.set_width(modal_width(ui.ctx(), 760.0));
                        let outcome = egui::ScrollArea::vertical().max_height(max_h).show(ui, |ui| editor.ui(ui, &pal)).inner;
                        match outcome {
                            Some(Outcome::Done) => {
                                self.runs = editor.to_file();
                                if let Some(p) = &self.project {
                                    if let Err(e) = self.runs.save(&p.root) {
                                        log::warn!("Failed to save runs.json: {e}");
                                    }
                                }
                                if let Some(id) = editor.selected_id() {
                                    self.view.run_config = Some(id);
                                }
                                self.refresh_configs();
                                close = true;
                            }
                            Some(Outcome::Cancel) => close = true,
                            None => {}
                        }
                    }
                    Modal::Rename { index, name } => {
                        ui.set_width(modal_width(ui.ctx(), 380.0));
                        title(ui, t!("workspace-rename-title"));
                        let r = ui.add(TextEdit::singleline(name).desired_width(f32::INFINITY).margin(Margin::symmetric(10, 7)));
                        if focus {
                            r.request_focus();
                        }
                        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                        ui.add_space(16.0);
                        let mut ok = enter;
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ok |= primary(ui, t!("action-rename"));
                            if ui.add(egui::Button::new(t!("action-cancel")).min_size(vec2(84.0, 30.0))).clicked() {
                                close = true;
                            }
                        });
                        if ok {
                            rename_done = Some((*index, name.trim().to_owned()));
                        }
                    }
                }
            });
        // Окно настроек и импорт не закрываются щелчком мимо: там легко потерять изменения.
        // Escape закрывает их, как «Отмена».
        let sticky = matches!(modal, Modal::Settings(_) | Modal::Import(_) | Modal::RunConfigs(_));
        let escape = || resp.is_top_modal && !resp.any_popup_open && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape));
        if (sticky && escape()) || (!sticky && resp.should_close()) {
            close = true;
        }
        // Быструю смену темы закрыли без выбора — возвращаем прежнюю тему.
        if let Modal::ThemeSwitcher(sw) = &modal
            && close
            && !sw.committed
        {
            self.settings.appearance.theme = sw.original().to_owned();
        }
        if let Some((index, name)) = rename_done {
            if let Some(ws) = self.layout.workspaces.get_mut(index) {
                // Пустое имя возвращает название шаблона на текущем языке.
                ws.name = if name.is_empty() || (ws.builtin.is_some_and(|b| b.title() == name)) { None } else { Some(name) };
            }
            close = true;
        }
        // Пока шёл кадр, мог открыться другой диалог — он важнее.
        if !close && self.modal.is_none() {
            self.modal = Some(modal);
        }
    }

    pub(super) fn toast_ui(&mut self, ctx: &egui::Context) {
        let now = Self::now(ctx);
        let anim_id = Id::new("mycelium-toast-anim");
        let Some(t) = &self.toast else {
            ctx.animate_bool_with_time(anim_id, false, 0.0);
            return;
        };
        // Уведомление выезжает снизу и так же плавно исчезает.
        let visible = now <= t.until;
        let a = ctx.animate_bool_with_time(anim_id, visible, 0.22);
        if !visible && a <= 0.0 {
            self.toast = None;
            return;
        }
        let pal = &self.pal;
        let color = if t.error { pal.danger } else { pal.accent };
        egui::Area::new(Id::new("mycelium-toast"))
            .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -40.0 + (1.0 - a) * 18.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.multiply_opacity(a);
                Frame::new()
                    .fill(pal.toast_bg)
                    .stroke(Stroke::new(1.0, color.gamma_multiply(0.6)))
                    .corner_radius(CornerRadius::same(12))
                    .inner_margin(Margin::symmetric(18, 11))
                    .shadow(egui::Shadow { offset: [0, 6], blur: 22, spread: 0, color: pal.shadow })
                    .show(ui, |ui| {
                        // Длинное уведомление переносится, а не уходит за края окна.
                        ui.set_max_width((ctx.content_rect().width() - 80.0).clamp(120.0, 640.0));
                        ui.add(egui::Label::new(RichText::new(&t.text).color(pal.toast_text)).wrap());
                    });
            });
        ctx.request_repaint_after_secs(if a < 1.0 { 0.0 } else { 0.5 });
    }
}

/// Ширина диалога: желаемая, но не шире окна.
pub(super) fn modal_width(ctx: &egui::Context, want: f32) -> f32 {
    want.min(ctx.content_rect().width() - 2.0 * 22.0 - 40.0).max(160.0)
}

/// Заголовки меню верхней строки — по ним считается, помещаются ли меню целиком.
fn menu_titles() -> [String; 5] {
    [t!("menu-file"), t!("menu-view"), t!("menu-run"), t!("menu-window"), t!("menu-help")]
}

/// Пункт меню с сочетанием клавиш справа.
fn item(text: String, shortcut: String) -> egui::Button<'static> {
    egui::Button::new(text).shortcut_text(shortcut)
}

/// Пункт меню со значком слева и сочетанием клавиш справа.
fn icon_item(ui: &Ui, icon: &str, text: String, shortcut: String) -> egui::Button<'static> {
    let img = forma_ui::icons::image(ui.ctx(), icon, forma_ui::icons::SIZE - 2.0, ui.visuals().widgets.inactive.fg_stroke.color);
    egui::Button::new((img, text)).shortcut_text(shortcut)
}

/// Ширина вкладки рабочего пространства с названием `title`.
fn workspace_tab_width(ui: &Ui, title: &str) -> f32 {
    ui.painter().layout_no_wrap(title.to_owned(), FontId::proportional(13.0), Color32::WHITE).size().x + 22.0
}

/// Вкладка рабочего пространства: «таблетка», выбранная подсвечена акцентом.
/// Не шире `max_width`: длинное название обрезается многоточием.
fn workspace_tab(ui: &mut Ui, pal: &Palette, id: Id, title: &str, active: bool, max_width: f32) -> egui::Response {
    let font = FontId::proportional(13.0);
    let galley = widgets::elide(ui, title, font, pal.text, max_width - 22.0);
    let size = vec2(galley.size().x + 22.0, 24.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let resp = ui.interact(rect, id, Sense::click());
    let a = ui.ctx().animate_bool_with_time(id.with("active"), active, 0.18);
    let hover = ui.ctx().animate_bool_with_time(id.with("hover"), resp.hovered(), 0.12);
    let painter = ui.painter();
    if hover > 0.0 {
        painter.rect_filled(rect, CornerRadius::same(7), pal.workspace_tab_hover.gamma_multiply(hover * (1.0 - a)));
    }
    if a > 0.0 {
        painter.rect_filled(rect, CornerRadius::same(7), pal.workspace_tab_active.gamma_multiply(a));
    }
    let color = pal.workspace_tab_text.lerp_to_gamma(pal.workspace_tab_active_text, a.max(hover * 0.6));
    painter.galley(rect.center() - galley.size() / 2.0, galley, color);
    resp.on_hover_cursor(CursorIcon::PointingHand)
}
