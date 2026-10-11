//! Оформление главного окна: верхняя строка с меню, рабочими пространствами и запуском,
//! строка состояния, экран приветствия, диалоги и уведомления.

use std::path::PathBuf;

use eframe::egui::{self, Align, Align2, Color32, CornerRadius, CursorIcon, FontId, Frame, Id, Key, Layout, Margin, RichText, Sense, Stroke, TextEdit, Ui, vec2};
use forma_i18n::t;

use super::run_configs::Outcome;
use super::{APP_TITLE, Cmd, Modal, MyceliumApp};
use crate::actions;
use crate::editors::EditorKind;
use forma_ui::theme::{self, Accent, Palette, ThemeKind};
use forma_ui::widgets;
use crate::workspace::Builtin;

/// Иконка нужного размера в пикселях экрана — так она остаётся чёткой.
pub(super) fn icon_texture(ctx: &egui::Context, cache: &mut Option<(u32, egui::TextureHandle)>, points: f32, name: &str) -> egui::TextureHandle {
    let px = (points * ctx.pixels_per_point()).round().max(16.0) as u32;
    if let Some((size, tex)) = cache {
        if *size == px {
            return tex.clone();
        }
    }
    let img = egui::ColorImage::from_rgba_unmultiplied([px as usize, px as usize], &crate::icon::rgba(px));
    let tex = ctx.load_texture(name, img, egui::TextureOptions::LINEAR);
    *cache = Some((px, tex.clone()));
    tex
}

impl MyceliumApp {
    pub(super) fn top_bar(&mut self, ui: &mut Ui) {
        let pal = self.pal.clone();
        let mark = icon_texture(ui.ctx(), &mut self.mark, 20.0, "mycelium-mark");
        egui::Panel::top("mycelium-top").frame(Frame::new().fill(pal.bg_panel).inner_margin(Margin::symmetric(8, 4))).show(ui, |ui| {
            egui::MenuBar::new().style(theme::menu_bar_style).config(theme::menu_config()).ui(ui, |ui| {
                ui.add(egui::Image::new(egui::load::SizedTexture::new(mark.id(), vec2(20.0, 20.0))));
                ui.add_space(4.0);
                self.menus(ui, &pal);
                ui.add_space(18.0);
                self.workspace_tabs(ui, &pal);
                if self.project.is_some() {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.horizontal(|ui| {
                            crate::editors::console::run_bar(ui, &pal, &self.console, &self.configs, &mut self.view.run_config, &mut self.cmds);
                        });
                    });
                }
            });
        });
    }

    fn menus(&mut self, ui: &mut Ui, pal: &Palette) {
        let item = |text: String, shortcut: String| egui::Button::new(text).shortcut_text(shortcut);
        widgets::top_menu(ui, &t!("menu-file"), false, |ui| {
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
        });
        // «Вид» не закрывается от переключателей, чтобы можно было поменять сразу несколько.
        widgets::top_menu(ui, &t!("menu-view"), true, |ui| {
            ui.menu_button(t!("settings-theme"), |ui| {
                for (kind, label) in [(ThemeKind::Dark, t!("theme-dark")), (ThemeKind::Light, t!("theme-light"))] {
                    if ui.add(egui::Button::new(label).selected(self.settings.appearance.theme == kind)).clicked() {
                        self.settings.appearance.theme = kind;
                    }
                }
            });
            ui.menu_button(t!("settings-accent"), |ui| {
                for a in Accent::ALL {
                    let text = RichText::new(format!("●  {}", a.label())).color(a.color(pal.dark));
                    if ui.add(egui::Button::new(text).selected(self.settings.appearance.accent == a)).clicked() {
                        self.settings.appearance.accent = a;
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.add_space(theme::MENU_PAD_X);
                ui.label(t!("settings-font-size"));
                ui.add(egui::Slider::new(&mut self.settings.appearance.font_size, 13.0..=22.0).step_by(0.5));
                ui.add_space(theme::MENU_PAD_X);
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
        });
        widgets::top_menu(ui, &t!("menu-run"), false, |ui| {
            if self.configs.is_empty() {
                ui.label(RichText::new(t!("run-no-configs")).color(pal.text_faint));
            }
            for c in &self.configs {
                // Идущий запуск помечен точкой; повторный выбор перезапускает его.
                let running = self.console.is_running_config(&c.id);
                let text = format!("{}  {}{}", c.kind.icon(), c.label(), if running { "  ●" } else { "" });
                if ui.button(text).on_hover_text(c.command_line()).clicked() {
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
            if ui.add_enabled(self.project.is_some(), item(format!("✎  {}", t!("run-edit-configs")), self.keys(actions::EDIT_RUN_CONFIGS))).clicked() {
                self.cmds.push(Cmd::EditRunConfigs);
            }
        });
        widgets::top_menu(ui, &t!("menu-window"), false, |ui| {
            ui.menu_button(t!("menu-open-editor"), |ui| {
                for kind in EditorKind::ALL {
                    if ui.button(format!("{}  {}", kind.icon(), kind.title())).clicked() {
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
        });
        widgets::top_menu(ui, &t!("menu-help"), false, |ui| {
            if ui.add(item(t!("action-find-action"), self.keys(actions::FIND_ACTION))).clicked() {
                self.cmds.push(Cmd::FindAction);
            }
            ui.separator();
            if ui.add(item(t!("action-about"), self.keys(actions::ABOUT))).clicked() {
                self.cmds.push(Cmd::About);
            }
        });
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

    /// Вкладки рабочих пространств в верхней строке, как в Blender.
    fn workspace_tabs(&mut self, ui: &mut Ui, pal: &Palette) {
        let count = self.layout.workspaces.len();
        for i in 0..count {
            let title = self.layout.workspaces[i].title();
            let resp = workspace_tab(ui, pal, Id::new(("mycelium-workspace", i)), &title, i == self.layout.active);
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
        let plus = ui.add(egui::Button::new(RichText::new("+").size(15.0).color(pal.text_muted)).frame(false)).on_hover_text(t!("menu-new-workspace"));
        egui::Popup::menu(&plus).style(theme::menu_style).show(|ui| self.new_workspace_items(ui));
    }

    pub(super) fn status_bar(&mut self, ui: &mut Ui) {
        let pal = self.pal.clone();
        egui::Panel::bottom("mycelium-status").frame(Frame::new().fill(pal.bg_panel).inner_margin(Margin::symmetric(12, 3))).show(ui, |ui| {
            ui.horizontal(|ui| {
                let small = |s: String, c: Color32| RichText::new(s).size(12.0).color(c);
                match &self.project {
                    Some(p) => {
                        ui.label(small("●".into(), pal.success));
                        ui.label(small(p.name().to_owned(), pal.text_muted)).on_hover_text(p.root.display().to_string());
                        let info = &p.info;
                        if info.is_mod() {
                            let loaders: Vec<&str> = info.loaders.iter().map(|l| l.name()).collect();
                            let mut text = loaders.join(" + ");
                            if let Some(mc) = &info.minecraft {
                                text.push_str(&format!(" · {mc}"));
                            }
                            ui.label(small(text, pal.text_faint));
                        }
                    }
                    None => {
                        ui.label(small(t!("status-no-project"), pal.text_faint));
                    }
                }
                if self.sync.is_busy() {
                    ui.label(small(format!("⟳ {}…", t!("status-syncing")), pal.accent));
                }
                if let Some(lsp) = &self.lsp {
                    let (text, color) = match &lsp.status {
                        crate::lsp::LspStatus::Ready => match lsp.problem_counts() {
                            (0, 0) => ("☕ Java ✔".to_owned(), pal.text_faint),
                            (e, w) => (format!("☕ ✖ {e}  ⚠ {w}"), if e > 0 { pal.danger } else { pal.warning }),
                        },
                        crate::lsp::LspStatus::NotInstalled => (format!("☕ {}", t!("status-lsp-off")), pal.text_faint),
                        crate::lsp::LspStatus::Failed(_) | crate::lsp::LspStatus::NoJdk(_) => (format!("☕ {}", t!("status-lsp-error")), pal.danger),
                        _ => (format!("☕ {}…", t!("status-lsp-busy")), pal.accent),
                    };
                    ui.label(small(text, color)).on_hover_text(t!("status-lsp-hint"));
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
                    let resp = ui.add(egui::Label::new(small(format!("▶ {}", running.join("  ·  ")), pal.accent)).sense(Sense::click()));
                    if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                        self.cmds.push(Cmd::OpenEditor(EditorKind::Console));
                    }
                }
                // Безобидный шум Minecraft в счётчик не входит.
                let problems: Vec<_> = self.console.problems.problems().iter().filter(|p| !p.sample.noise).collect();
                if !problems.is_empty() {
                    let errors = problems.iter().filter(|p| p.sample.crash || p.sample.level >= mycelium_log::Level::Error || p.sample.throwable.is_some()).count();
                    let warnings = problems.len() - errors;
                    let resp = ui.add(egui::Label::new(RichText::new(format!("✖ {errors}  ⚠ {warnings}")).size(12.0).color(if errors > 0 { pal.danger } else { pal.warning })).sense(Sense::click()));
                    if resp.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(t!("status-problems")).clicked() {
                        self.cmds.push(Cmd::OpenEditor(EditorKind::Exceptions));
                    }
                }
                let windows = self.layout.dock.windows.len();
                if windows > 0 {
                    ui.label(small("·".into(), pal.text_faint));
                    ui.label(small(t!("status-detached-windows", count = windows), pal.text_faint));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(small(self.gpu_info.clone(), pal.text_faint)).on_hover_text(t!("status-renderer"));
                    ui.label(small("·".into(), pal.text_faint));
                    ui.label(small(forma_i18n::language().to_uppercase(), pal.text_faint)).on_hover_text(t!("settings-language"));
                });
            });
        });
    }

    pub(super) fn welcome_ui(&mut self, ui: &mut Ui) {
        let pal = self.pal.clone();
        let logo = icon_texture(ui.ctx(), &mut self.logo, 104.0, "mycelium-logo");
        egui::CentralPanel::default().frame(Frame::new().fill(pal.bg)).show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space((ui.available_height() * 0.14).max(24.0));
                ui.add(egui::Image::new(egui::load::SizedTexture::new(logo.id(), vec2(104.0, 104.0))));
                ui.add_space(14.0);
                // «FORMA» вразрядку над названием, как подпись студии.
                ui.label(RichText::new("F O R M A").size(13.0).family(forma_ui::fonts::semibold()).color(pal.accent));
                ui.add_space(2.0);
                ui.label(RichText::new("Mycelium").size(38.0).family(forma_ui::fonts::bold()).color(pal.heading));
                ui.add_space(6.0);
                widgets::centered_text(ui, &t!("welcome-tagline"), 15.0, pal.text_muted, 460.0);
                ui.add_space(26.0);
                let big = |text: String| egui::Button::new(RichText::new(text).size(15.5)).min_size(vec2(300.0, 40.0)).corner_radius(10);
                if ui.add(big(format!("📂  {}", t!("action-open-project"))).fill(pal.accent_soft)).clicked() {
                    self.cmds.push(Cmd::OpenProjectDialog);
                }
                ui.add_space(6.0);
                if ui.add(big(format!("➜  {}", t!("welcome-continue")))).clicked() {
                    self.cmds.push(Cmd::ContinueWithoutProject);
                }
                let recent: Vec<PathBuf> = self.settings.recent.projects.iter().filter(|p| p.is_dir()).cloned().collect();
                if !recent.is_empty() {
                    ui.add_space(26.0);
                    ui.label(RichText::new(t!("welcome-recent")).size(13.0).color(pal.text_faint));
                    ui.add_space(4.0);
                    for p in recent.into_iter().take(6) {
                        let name = p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                        let resp = ui.add(egui::Button::new(RichText::new(&name).size(14.5)).frame(false)).on_hover_text(p.display().to_string());
                        if resp.clicked() {
                            self.cmds.push(Cmd::OpenProject(p));
                        }
                    }
                }
                ui.add_space(26.0);
                if ui.add(egui::Button::new(RichText::new(format!("⚙  {}", t!("action-settings"))).size(13.0).color(pal.text_muted)).frame(false)).clicked() {
                    self.cmds.push(Cmd::Settings);
                }
                ui.add_space(4.0);
                ui.label(RichText::new(format!("{} · {}", t!("status-renderer"), self.gpu_info)).size(12.0).color(pal.text_faint));
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
            .fill(pal.bg_elevated.gamma_multiply(t))
            .stroke(Stroke::new(1.0, pal.border.gamma_multiply(t)))
            .corner_radius(CornerRadius::same(14))
            .inner_margin(Margin::same(margin));
        let mut rename_done: Option<(usize, String)> = None;
        let resp = egui::Modal::new(Id::new("mycelium-modal"))
            .frame(frame)
            .backdrop_color(Color32::from_black_alpha((110.0 * t) as u8))
            .show(ctx, |ui| {
                ui.multiply_opacity(t);
                let title = |ui: &mut Ui, text: String| {
                    ui.label(RichText::new(text).size(18.0).family(forma_ui::fonts::semibold()).color(pal.heading));
                    ui.add_space(12.0);
                };
                let primary = |ui: &mut Ui, text: String| {
                    let text_color = if pal.dark { pal.bg } else { Color32::WHITE };
                    ui.add(egui::Button::new(RichText::new(text).color(text_color)).fill(pal.accent).min_size(vec2(96.0, 30.0))).clicked()
                };
                match &mut modal {
                    Modal::Settings(dialog) => close = self.settings_ui(ui, &pal, dialog),
                    Modal::FindAction(find) => close = self.find_action_ui(ui, &pal, find),
                    Modal::Export(export) => close = self.export_ui(ui, &pal, export, now),
                    Modal::Import(import) => close = self.import_ui(ui, &pal, import),
                    Modal::About => {
                        ui.set_width(380.0);
                        let logo = icon_texture(ui.ctx(), &mut self.logo, 104.0, "mycelium-logo");
                        ui.vertical_centered(|ui| {
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
                        });
                    }
                    Modal::RunConfigs(editor) => {
                        ui.set_width(760.0);
                        match editor.ui(ui, &pal) {
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
                        ui.set_width(380.0);
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
                    .fill(pal.bg_elevated)
                    .stroke(Stroke::new(1.0, color.gamma_multiply(0.6)))
                    .corner_radius(CornerRadius::same(12))
                    .inner_margin(Margin::symmetric(18, 11))
                    .shadow(egui::Shadow { offset: [0, 6], blur: 22, spread: 0, color: Color32::from_black_alpha(70) })
                    .show(ui, |ui| {
                        ui.label(RichText::new(&t.text).color(pal.text));
                    });
            });
        ctx.request_repaint_after_secs(if a < 1.0 { 0.0 } else { 0.5 });
    }
}

/// Вкладка рабочего пространства: «таблетка», выбранная подсвечена акцентом.
fn workspace_tab(ui: &mut Ui, pal: &Palette, id: Id, title: &str, active: bool) -> egui::Response {
    let font = FontId::proportional(13.0);
    let galley = ui.painter().layout_no_wrap(title.to_owned(), font, pal.text);
    let size = vec2(galley.size().x + 22.0, 24.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let resp = ui.interact(rect, id, Sense::click());
    let a = ui.ctx().animate_bool_with_time(id.with("active"), active, 0.18);
    let hover = ui.ctx().animate_bool_with_time(id.with("hover"), resp.hovered(), 0.12);
    let painter = ui.painter();
    if hover > 0.0 {
        painter.rect_filled(rect, CornerRadius::same(7), pal.bg_hover.gamma_multiply(hover * (1.0 - a)));
    }
    if a > 0.0 {
        painter.rect_filled(rect, CornerRadius::same(7), pal.accent_soft.gamma_multiply(a));
    }
    let color = pal.text_muted.lerp_to_gamma(pal.heading, a.max(hover * 0.6));
    painter.galley(rect.center() - galley.size() / 2.0, galley, color);
    resp.on_hover_cursor(CursorIcon::PointingHand)
}
