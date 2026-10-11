//! Главное окно Forma Mycelium: меню, рабочие пространства, области, строка состояния,
//! экран приветствия и диалоги.

use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, CursorIcon, FontId, Frame, Id, Key, KeyboardShortcut, Layout, Margin, Modifiers, RichText,
    Sense, Stroke, TextEdit, Ui, vec2,
};
use mycelium_i18n::t;
use mycelium_layout::{DockState, DockStyle};
use serde::Serialize;

use crate::editors::{EditorKind, EditorTab, Editors, ProjectTree};
use crate::project::Project;
use crate::settings::{self, Settings};
use crate::theme::{self, Accent, Palette, ThemeKind};
use crate::widgets;
use crate::workspace::{self, Builtin, Ids, LAYOUT_VERSION, LayoutFile, Workspace};

pub const APP_TITLE: &str = "Forma Mycelium";

const SETTINGS_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::CTRL.plus(Modifiers::ALT), Key::S);

/// Действия, которые копятся за кадр и выполняются в его конце.
pub enum Cmd {
    OpenProjectDialog,
    OpenProject(PathBuf),
    CloseProject,
    ContinueWithoutProject,
    OpenEditor(EditorKind),
    SwitchWorkspace(usize),
    NewWorkspace(Option<Builtin>),
    DuplicateWorkspace(usize),
    RenameWorkspace(usize),
    ResetWorkspace(usize),
    DeleteWorkspace(usize),
    ResetAllWorkspaces,
    DetachFocused,
    ReturnAllWindows,
    ToggleMaximize,
    Settings,
    About,
    Quit,
}

enum Modal {
    Settings,
    About,
    Rename { index: usize, name: String },
}

struct Toast {
    text: String,
    error: bool,
    until: f64,
}

/// Раскладка для записи в файл (поля совпадают с [`LayoutFile`]).
#[derive(Serialize)]
struct LayoutRef<'a> {
    version: u32,
    workspaces: &'a [Workspace],
    active: usize,
    dock: &'a DockState<EditorTab>,
    next_tab_id: u64,
}

pub struct MyceliumApp {
    settings: Settings,
    pal: Palette,
    style_key: Option<(ThemeKind, Accent, u32)>,
    icon: Arc<egui::IconData>,
    dock_style: DockStyle,
    workspaces: Vec<Workspace>,
    active_ws: usize,
    dock: DockState<EditorTab>,
    next_tab_id: u64,
    project: Option<Project>,
    tree: ProjectTree,
    /// Экран приветствия вместо рабочего интерфейса.
    welcome: bool,
    modal: Option<Modal>,
    modal_focus: bool,
    toast: Option<Toast>,
    cmds: Vec<Cmd>,
    gpu_info: String,
    window_title: String,
    logo: Option<(u32, egui::TextureHandle)>,
    mark: Option<(u32, egui::TextureHandle)>,
}

impl MyceliumApp {
    pub fn new(cc: &eframe::CreationContext<'_>, settings: Settings, project_arg: Option<PathBuf>) -> Self {
        let ctx = &cc.egui_ctx;
        crate::fonts::install(ctx);
        ctx.set_zoom_factor(settings.zoom.clamp(0.5, 3.0));
        let pal = Palette::new(settings.theme, settings.accent);
        let icon = Arc::new(theme::app_icon());
        let dock_style = pal.dock_style(APP_TITLE, icon.clone());
        let gpu_info = cc
            .wgpu_render_state
            .as_ref()
            .map(|rs| {
                let info = rs.adapter.get_info();
                format!("{:?} · {}", info.backend, info.name)
            })
            .unwrap_or_else(|| "Vulkan".into());

        let (workspaces, active_ws, dock, next_tab_id) = load_layout();
        let mut app = Self {
            settings,
            pal,
            style_key: None,
            icon,
            dock_style,
            workspaces,
            active_ws,
            dock,
            next_tab_id,
            project: None,
            tree: ProjectTree::default(),
            welcome: true,
            modal: None,
            modal_focus: false,
            toast: None,
            cmds: Vec::new(),
            gpu_info,
            window_title: String::new(),
            logo: None,
            mark: None,
        };
        // Как в IDEA: при запуске снова открывается последний проект.
        let path = project_arg.or_else(|| app.settings.last_project.clone().filter(|p| p.is_dir()));
        if let Some(path) = path {
            app.open_project(path, 0.0);
        }
        app
    }

    fn now(ctx: &egui::Context) -> f64 {
        ctx.input(|i| i.time)
    }

    fn toast(&mut self, text: impl Into<String>, error: bool, now: f64) {
        self.toast = Some(Toast { text: text.into(), error, until: now + if error { 5.0 } else { 2.6 } });
    }

    fn open_project(&mut self, path: PathBuf, now: f64) {
        match Project::open(path.clone()) {
            Ok(project) => {
                self.settings.remember_project(&path);
                self.project = Some(project);
                self.tree = ProjectTree::default();
                self.welcome = false;
            }
            Err(shown) => {
                self.settings.recent_projects.retain(|p| p != &path);
                if self.settings.last_project.as_ref() == Some(&path) {
                    self.settings.last_project = None;
                }
                self.toast(t!("toast-project-missing", path = shown), true, now);
            }
        }
    }

    fn persist(&mut self) {
        self.settings.save();
        if let Some(dir) = settings::config_dir() {
            let layout = LayoutRef {
                version: LAYOUT_VERSION,
                workspaces: &self.workspaces,
                active: self.active_ws,
                dock: &self.dock,
                next_tab_id: self.next_tab_id,
            };
            settings::write_json(&dir.join("layout.json"), &layout);
        }
    }

    fn ids(&mut self) -> Ids<'_> {
        Ids { dock: &mut self.dock, next_tab: &mut self.next_tab_id }
    }

    fn process_cmds(&mut self, ctx: &egui::Context) {
        let now = Self::now(ctx);
        for cmd in std::mem::take(&mut self.cmds) {
            match cmd {
                Cmd::OpenProjectDialog => {
                    let mut dialog = rfd::FileDialog::new().set_title(t!("dialog-open-project"));
                    if let Some(dir) = self.settings.recent_projects.first().and_then(|p| p.parent()) {
                        dialog = dialog.set_directory(dir);
                    }
                    if let Some(dir) = dialog.pick_folder() {
                        self.open_project(dir, now);
                    }
                }
                Cmd::OpenProject(path) => self.open_project(path, now),
                Cmd::CloseProject => {
                    self.project = None;
                    self.settings.last_project = None;
                    self.welcome = true;
                }
                Cmd::ContinueWithoutProject => self.welcome = false,
                Cmd::OpenEditor(kind) => {
                    let tab = EditorTab::new(kind, &mut self.next_tab_id);
                    self.dock.open_tab(tab);
                }
                Cmd::SwitchWorkspace(i) => {
                    if i < self.workspaces.len() {
                        self.active_ws = i;
                    }
                }
                Cmd::NewWorkspace(template) => {
                    let ws = match template {
                        Some(b) => Workspace { builtin: Some(b), name: None, surface: workspace::builtin_surface(b, &mut self.ids()) },
                        None => self.copy_workspace(self.active_ws),
                    };
                    self.workspaces.push(ws);
                    self.active_ws = self.workspaces.len() - 1;
                }
                Cmd::DuplicateWorkspace(i) => {
                    if i < self.workspaces.len() {
                        let ws = self.copy_workspace(i);
                        self.workspaces.insert(i + 1, ws);
                        self.active_ws = i + 1;
                    }
                }
                Cmd::RenameWorkspace(i) => {
                    if let Some(ws) = self.workspaces.get(i) {
                        self.modal = Some(Modal::Rename { index: i, name: ws.title() });
                        self.modal_focus = true;
                    }
                }
                Cmd::ResetWorkspace(i) => {
                    if let Some(b) = self.workspaces.get(i).and_then(|w| w.builtin) {
                        let surface = workspace::builtin_surface(b, &mut self.ids());
                        self.workspaces[i].surface = surface;
                    }
                }
                Cmd::DeleteWorkspace(i) => {
                    if self.workspaces.len() > 1 && i < self.workspaces.len() {
                        self.workspaces.remove(i);
                        if self.active_ws > i || self.active_ws >= self.workspaces.len() {
                            self.active_ws = self.active_ws.saturating_sub(1);
                        }
                    }
                }
                Cmd::ResetAllWorkspaces => {
                    self.workspaces = workspace::default_workspaces(&mut self.ids());
                    self.active_ws = 0;
                }
                Cmd::DetachFocused => self.dock.detach_focused(&self.workspaces[self.active_ws].surface),
                Cmd::ReturnAllWindows => self.dock.return_all(),
                Cmd::ToggleMaximize => self.dock.toggle_maximize_focused(&self.workspaces[self.active_ws].surface),
                Cmd::Settings => self.modal = Some(Modal::Settings),
                Cmd::About => self.modal = Some(Modal::About),
                Cmd::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            }
        }
    }

    /// Копия пространства с новыми номерами областей и вкладок.
    fn copy_workspace(&mut self, i: usize) -> Workspace {
        let mut ws = self.workspaces[i].clone();
        ws.name = Some(t!("workspace-copy-name", name = ws.title()));
        let dock = &mut self.dock;
        ws.surface.reassign_ids(|| dock.new_area_id());
        let next = &mut self.next_tab_id;
        ws.surface.for_each_area_mut(|a| {
            for tab in &mut a.tabs {
                *next += 1;
                tab.id = *next;
            }
        });
        ws
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|i| i.consume_shortcut(&SETTINGS_SHORTCUT)) {
            self.modal = Some(Modal::Settings);
        }
    }

    fn apply_style(&mut self, ctx: &egui::Context) {
        let key = (self.settings.theme, self.settings.accent, self.settings.font_size.to_bits());
        if self.style_key != Some(key) {
            self.pal = Palette::new(self.settings.theme, self.settings.accent);
            self.dock_style = self.pal.dock_style(APP_TITLE, self.icon.clone());
            theme::apply(ctx, &self.pal, self.settings.font_size);
            self.style_key = Some(key);
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let title = match &self.project {
            Some(p) => format!("{} — {APP_TITLE}", p.name),
            None => APP_TITLE.to_owned(),
        };
        if title != self.window_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }
    }
}

/// Раскладка из файла или исходная, если файла нет или он испорчен.
fn load_layout() -> (Vec<Workspace>, usize, DockState<EditorTab>, u64) {
    let file = settings::config_dir()
        .and_then(|d| settings::read_json::<LayoutFile>(&d.join("layout.json")))
        .filter(|f| f.version == LAYOUT_VERSION && !f.workspaces.is_empty());
    match file {
        Some(mut f) => {
            for w in &mut f.workspaces {
                w.surface.sanitize();
            }
            f.dock.sanitize();
            let max_area = f.workspaces.iter().map(|w| w.surface.max_area_id()).max().unwrap_or(0);
            f.dock.reserve_ids(max_area);
            let surfaces = f.workspaces.iter().map(|w| &w.surface).chain(f.dock.windows.iter().map(|w| &w.surface));
            let max_tab = surfaces.flat_map(|s| s.areas().into_iter().flat_map(|a| a.tabs.iter().map(|t| t.id))).max().unwrap_or(0);
            let active = f.active.min(f.workspaces.len() - 1);
            (f.workspaces, active, f.dock, f.next_tab_id.max(max_tab))
        }
        None => {
            let mut dock = DockState::default();
            let mut next_tab = 0;
            let spaces = workspace::default_workspaces(&mut Ids { dock: &mut dock, next_tab: &mut next_tab });
            (spaces, 0, dock, next_tab)
        }
    }
}

/// Иконка нужного размера в пикселях экрана — так она остаётся чёткой.
fn icon_texture(ctx: &egui::Context, cache: &mut Option<(u32, egui::TextureHandle)>, points: f32, name: &str) -> egui::TextureHandle {
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

// ============================================================================================
// Цикл отрисовки
// ============================================================================================

impl eframe::App for MyceliumApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.apply_style(&ctx);
        self.handle_keys(&ctx);
        self.settings.zoom = ctx.zoom_factor();
        if self.active_ws >= self.workspaces.len() {
            self.active_ws = 0;
        }

        if self.welcome {
            self.welcome_ui(ui);
        } else {
            self.top_bar(ui);
            self.status_bar(ui);
            self.dock_ui(ui, &ctx);
        }
        self.modals(&ctx);
        self.toast_ui(&ctx);
        self.process_cmds(&ctx);
        self.update_title(&ctx);
    }

    /// eframe вызывает это раз в полминуты и при выходе.
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        self.persist();
    }

    fn on_exit(&mut self) {
        self.persist();
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.pal.bg_deep.to_normalized_gamma_f32()
    }
}

// ============================================================================================
// Части окна
// ============================================================================================

impl MyceliumApp {
    fn dock_ui(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let ws = &mut self.workspaces[self.active_ws];
        let mut viewer = Editors {
            pal: &self.pal,
            project: self.project.as_ref(),
            tree: &mut self.tree,
            recent: &self.settings.recent_projects,
            next_tab_id: &mut self.next_tab_id,
            cmds: &mut self.cmds,
        };
        let dock = &mut self.dock;
        let style = &self.dock_style;
        egui::CentralPanel::default().frame(Frame::new().fill(self.pal.bg_deep)).show(ui, |ui| {
            dock.show_main(ui, &mut ws.surface, &mut viewer, style);
        });
        dock.show_windows(ctx, &mut viewer, style);
        dock.finish(ctx, &mut ws.surface, &mut viewer);
    }

    fn top_bar(&mut self, ui: &mut Ui) {
        let pal = self.pal.clone();
        let mark = icon_texture(ui.ctx(), &mut self.mark, 20.0, "mycelium-mark");
        egui::Panel::top("mycelium-top").frame(Frame::new().fill(pal.bg_panel).inner_margin(Margin::symmetric(8, 4))).show(ui, |ui| {
            egui::MenuBar::new().style(theme::menu_bar_style).config(theme::menu_config()).ui(ui, |ui| {
                ui.add(egui::Image::new(egui::load::SizedTexture::new(mark.id(), vec2(20.0, 20.0))));
                ui.add_space(4.0);
                self.menus(ui, &pal);
                ui.add_space(18.0);
                self.workspace_tabs(ui, &pal);
            });
        });
    }

    fn menus(&mut self, ui: &mut Ui, pal: &Palette) {
        let item = |text: String, shortcut: &str| egui::Button::new(text).shortcut_text(shortcut);
        widgets::top_menu(ui, &t!("menu-file"), false, |ui| {
            if ui.add(item(t!("action-open-project"), "")).clicked() {
                self.cmds.push(Cmd::OpenProjectDialog);
            }
            ui.menu_button(t!("menu-recent-projects"), |ui| {
                let recent: Vec<PathBuf> = self.settings.recent_projects.iter().filter(|p| p.is_dir()).cloned().collect();
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
            if ui.add_enabled(self.project.is_some(), item(t!("action-close-project"), "")).clicked() {
                self.cmds.push(Cmd::CloseProject);
            }
            ui.separator();
            if ui.add(item(t!("action-settings"), "Ctrl+Alt+S")).clicked() {
                self.cmds.push(Cmd::Settings);
            }
            ui.separator();
            if ui.add(item(t!("action-quit"), "Alt+F4")).clicked() {
                self.cmds.push(Cmd::Quit);
            }
        });
        // «Вид» не закрывается от переключателей, чтобы можно было поменять сразу несколько.
        widgets::top_menu(ui, &t!("menu-view"), true, |ui| {
            ui.menu_button(t!("settings-theme"), |ui| {
                for (kind, label) in [(ThemeKind::Dark, t!("theme-dark")), (ThemeKind::Light, t!("theme-light"))] {
                    if ui.add(egui::Button::new(label).selected(self.settings.theme == kind)).clicked() {
                        self.settings.theme = kind;
                    }
                }
            });
            ui.menu_button(t!("settings-accent"), |ui| {
                for a in Accent::ALL {
                    let text = RichText::new(format!("●  {}", a.label())).color(a.color(pal.dark));
                    if ui.add(egui::Button::new(text).selected(self.settings.accent == a)).clicked() {
                        self.settings.accent = a;
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.add_space(theme::MENU_PAD_X);
                ui.label(t!("settings-font-size"));
                ui.add(egui::Slider::new(&mut self.settings.font_size, 13.0..=22.0).step_by(0.5));
                ui.add_space(theme::MENU_PAD_X);
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.add_space(theme::MENU_PAD_X);
                ui.label(RichText::new(t!("view-zoom-hint")).size(12.0).color(pal.text_faint));
            });
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
            if ui.add(item(t!("dock-maximize"), mycelium_layout::MAXIMIZE_SHORTCUT_TEXT)).clicked() {
                self.cmds.push(Cmd::ToggleMaximize);
            }
            if ui.add(item(t!("dock-detach"), "")).clicked() {
                self.cmds.push(Cmd::DetachFocused);
            }
            if ui.add_enabled(!self.dock.windows.is_empty(), item(t!("action-return-windows"), "")).clicked() {
                self.cmds.push(Cmd::ReturnAllWindows);
            }
            ui.separator();
            ui.menu_button(t!("menu-new-workspace"), |ui| self.new_workspace_items(ui));
            let builtin = self.workspaces[self.active_ws].builtin.is_some();
            if ui.add_enabled(builtin, item(t!("action-reset-workspace"), "")).clicked() {
                self.cmds.push(Cmd::ResetWorkspace(self.active_ws));
            }
            if ui.add(item(t!("action-reset-all-workspaces"), "")).clicked() {
                self.cmds.push(Cmd::ResetAllWorkspaces);
            }
        });
        widgets::top_menu(ui, &t!("menu-help"), false, |ui| {
            if ui.button(t!("action-about")).clicked() {
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
        let count = self.workspaces.len();
        for i in 0..count {
            let title = self.workspaces[i].title();
            let resp = workspace_tab(ui, pal, Id::new(("mycelium-workspace", i)), &title, i == self.active_ws);
            if resp.double_clicked() {
                self.cmds.push(Cmd::RenameWorkspace(i));
            } else if resp.clicked() {
                self.cmds.push(Cmd::SwitchWorkspace(i));
            }
            let builtin = self.workspaces[i].builtin.is_some();
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

    fn status_bar(&mut self, ui: &mut Ui) {
        let pal = self.pal.clone();
        egui::Panel::bottom("mycelium-status").frame(Frame::new().fill(pal.bg_panel).inner_margin(Margin::symmetric(12, 3))).show(ui, |ui| {
            ui.horizontal(|ui| {
                let small = |s: String, c: Color32| RichText::new(s).size(12.0).color(c);
                match &self.project {
                    Some(p) => {
                        ui.label(small("●".into(), pal.success));
                        ui.label(small(p.name.clone(), pal.text_muted)).on_hover_text(p.root.display().to_string());
                    }
                    None => {
                        ui.label(small(t!("status-no-project"), pal.text_faint));
                    }
                }
                let windows = self.dock.windows.len();
                if windows > 0 {
                    ui.label(small("·".into(), pal.text_faint));
                    ui.label(small(t!("status-detached-windows", count = windows), pal.text_faint));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(small(self.gpu_info.clone(), pal.text_faint)).on_hover_text(t!("status-renderer"));
                    ui.label(small("·".into(), pal.text_faint));
                    ui.label(small(mycelium_i18n::language().to_uppercase(), pal.text_faint)).on_hover_text(t!("settings-language"));
                });
            });
        });
    }

    fn welcome_ui(&mut self, ui: &mut Ui) {
        let pal = self.pal.clone();
        let logo = icon_texture(ui.ctx(), &mut self.logo, 104.0, "mycelium-logo");
        egui::CentralPanel::default().frame(Frame::new().fill(pal.bg)).show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space((ui.available_height() * 0.14).max(24.0));
                ui.add(egui::Image::new(egui::load::SizedTexture::new(logo.id(), vec2(104.0, 104.0))));
                ui.add_space(14.0);
                // «FORMA» вразрядку над названием, как подпись студии.
                ui.label(RichText::new("F O R M A").size(13.0).family(crate::fonts::semibold()).color(pal.accent));
                ui.add_space(2.0);
                ui.label(RichText::new("Mycelium").size(38.0).family(crate::fonts::bold()).color(pal.heading));
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
                let recent: Vec<PathBuf> = self.settings.recent_projects.iter().filter(|p| p.is_dir()).cloned().collect();
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

    fn modals(&mut self, ctx: &egui::Context) {
        let anim_id = Id::new("mycelium-modal-anim");
        // Диалог вынимаем на время отрисовки: внутри него меняются настройки приложения.
        let Some(mut modal) = self.modal.take() else {
            ctx.animate_bool_with_time(anim_id, false, 0.0);
            return;
        };
        // Диалог плавно проявляется вместе с затемнением фона.
        let t = ctx.animate_bool_with_time(anim_id, true, 0.16);
        let pal = self.pal.clone();
        let focus = std::mem::take(&mut self.modal_focus);
        let mut close = false;
        let frame = Frame::new()
            .fill(pal.bg_elevated.gamma_multiply(t))
            .stroke(Stroke::new(1.0, pal.border.gamma_multiply(t)))
            .corner_radius(CornerRadius::same(14))
            .inner_margin(Margin::same(22));
        let mut rename_done: Option<(usize, String)> = None;
        let resp = egui::Modal::new(Id::new("mycelium-modal"))
            .frame(frame)
            .backdrop_color(Color32::from_black_alpha((110.0 * t) as u8))
            .show(ctx, |ui| {
                ui.multiply_opacity(t);
                let title = |ui: &mut Ui, text: String| {
                    ui.label(RichText::new(text).size(18.0).family(crate::fonts::semibold()).color(pal.heading));
                    ui.add_space(12.0);
                };
                let primary = |ui: &mut Ui, text: String| {
                    let text_color = if pal.dark { pal.bg } else { Color32::WHITE };
                    ui.add(egui::Button::new(RichText::new(text).color(text_color)).fill(pal.accent).min_size(vec2(96.0, 30.0))).clicked()
                };
                match &mut modal {
                    Modal::Settings => {
                        ui.set_width(440.0);
                        title(ui, t!("settings-title"));
                        self.settings_body(ui, &pal);
                        ui.add_space(16.0);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if primary(ui, t!("action-done")) {
                                close = true;
                            }
                        });
                    }
                    Modal::About => {
                        ui.set_width(380.0);
                        let logo = icon_texture(ui.ctx(), &mut self.logo, 104.0, "mycelium-logo");
                        ui.vertical_centered(|ui| {
                            ui.add(egui::Image::new(egui::load::SizedTexture::new(logo.id(), vec2(72.0, 72.0))));
                            ui.add_space(8.0);
                            ui.label(RichText::new(APP_TITLE).size(20.0).family(crate::fonts::bold()).color(pal.heading));
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
        if resp.should_close() {
            close = true;
        }
        if let Some((index, name)) = rename_done {
            if let Some(ws) = self.workspaces.get_mut(index) {
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

    fn settings_body(&mut self, ui: &mut Ui, pal: &Palette) {
        let section = |ui: &mut Ui, text: String| {
            ui.add_space(6.0);
            ui.label(RichText::new(text).size(13.0).color(pal.text_muted));
            ui.add_space(2.0);
        };

        section(ui, t!("settings-language"));
        let languages = mycelium_i18n::languages();
        let mut labels = vec![t!("settings-language-system")];
        labels.extend(languages.iter().map(|(_, name)| name.clone()));
        let selected = match &self.settings.language {
            None => 0,
            Some(code) => languages.iter().position(|(c, _)| c == code).map_or(0, |i| i + 1),
        };
        let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        if let Some(i) = widgets::segmented(ui, pal, Id::new("settings-language"), &refs, selected, true) {
            self.settings.language = if i == 0 { None } else { Some(languages[i - 1].0.clone()) };
            mycelium_i18n::set_language(&self.settings.effective_language());
        }

        section(ui, t!("settings-theme"));
        let themes = [t!("theme-dark"), t!("theme-light")];
        let refs: Vec<&str> = themes.iter().map(String::as_str).collect();
        let selected = if self.settings.theme == ThemeKind::Dark { 0 } else { 1 };
        if let Some(i) = widgets::segmented(ui, pal, Id::new("settings-theme"), &refs, selected, true) {
            self.settings.theme = if i == 0 { ThemeKind::Dark } else { ThemeKind::Light };
        }

        section(ui, t!("settings-accent"));
        ui.horizontal(|ui| {
            for a in Accent::ALL {
                let (rect, resp) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
                let selected = self.settings.accent == a;
                let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.12);
                let c = a.color(pal.dark);
                ui.painter().circle_filled(rect.center(), 9.0 + hover * 1.5, c);
                if selected {
                    ui.painter().circle_stroke(rect.center(), 13.0, Stroke::new(2.0, c));
                }
                if resp.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(a.label()).clicked() {
                    self.settings.accent = a;
                }
            }
        });

        section(ui, t!("settings-font-size"));
        ui.add(egui::Slider::new(&mut self.settings.font_size, 13.0..=22.0).step_by(0.5));

        if let Some(dir) = settings::user_locales_dir() {
            ui.add_space(12.0);
            let hint = t!("settings-locales-hint", path = dir.display().to_string());
            ui.add(egui::Label::new(RichText::new(hint).size(11.5).color(pal.text_faint)).wrap());
        }
    }

    fn toast_ui(&mut self, ctx: &egui::Context) {
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
