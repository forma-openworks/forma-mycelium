//! Главное окно Forma Mycelium: проект, раскладка, редакторы, запуск и команды.
//!
//! Раскладка интерфейса принадлежит проекту и хранится в его `.mycelium/workspace.json`.
//! Проект без своей раскладки получает стандартную (её задаёт «Сохранить раскладку как
//! стандартную»), а если и её нет — заводскую. Без проекта используется отдельная раскладка
//! в папке настроек.

mod chrome;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui::{self, Frame, Key, KeyboardShortcut, Modifiers, Ui};
use mycelium_i18n::t;
use mycelium_layout::{DockStyle, Surface, SurfaceKey};
use mycelium_project::{Project, store};

use crate::editors::{Documents, EditorKind, EditorStates, EditorTab, Editors, ProjectTree};
use crate::run::{self, Console, RunConfig};
use crate::settings::{self, Settings};
use crate::lsp::{JavaLsp, LspEvent, LspStatus};
use crate::sync::{GradleSync, SyncResult};
use crate::theme::{self, Accent, Palette, ThemeKind};
use crate::workspace::{self, Builtin, Layout, ViewState, Workspace};

pub const APP_TITLE: &str = "Forma Mycelium";

const SETTINGS_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::CTRL.plus(Modifiers::ALT), Key::S);
const SAVE_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
/// Как в IDEA: Shift+F10 — запустить, Ctrl+F2 — остановить.
const RUN_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::SHIFT, Key::F10);
const STOP_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::F2);

/// Действия, которые копятся за кадр и выполняются в его конце.
pub enum Cmd {
    OpenProjectDialog,
    OpenProject(PathBuf),
    CloseProject,
    ContinueWithoutProject,
    OpenEditor(EditorKind),
    /// Открыть файл (путь абсолютный или относительно проекта), при желании на строке.
    OpenFile { path: PathBuf, line: Option<u32> },
    /// Открыть исходник класса из стек-трейса (своего мода или библиотеки).
    OpenSource { class: String, file: Option<String>, line: Option<u32> },
    /// Открыть файл из jar библиотеки (`.class` — его исходник).
    OpenLibraryEntry { lib: usize, entry: String },
    Reveal(PathBuf),
    /// Перечитать файлы проекта (загрузчик, версии, конфигурации запуска).
    Redetect,
    /// Заново выгрузить модель проекта из Gradle.
    SyncGradle,
    /// Скачать и установить jdtls.
    InstallJdtls,
    RestartJdtls,
    SaveAll,
    Run,
    Stop,
    Rerun,
    ClearConsole,
    ClearProblems,
    SwitchWorkspace(usize),
    NewWorkspace(Option<Builtin>),
    DuplicateWorkspace(usize),
    RenameWorkspace(usize),
    ResetWorkspace(usize),
    DeleteWorkspace(usize),
    ResetAllWorkspaces,
    SaveDefaultLayout,
    ApplyDefaultLayout,
    ResetDefaultLayout,
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

pub struct MyceliumApp {
    /// Контекст egui — фоновые задачи будят им интерфейс.
    ctx: egui::Context,
    settings: Settings,
    pal: Palette,
    style_key: Option<(ThemeKind, Accent, u32)>,
    icon: Arc<egui::IconData>,
    dock_style: DockStyle,
    layout: Layout,
    /// Что проект помнит об интерфейсе кроме раскладки (дерево, конфигурация запуска).
    view: ViewState,
    project: Option<Project>,
    configs: Vec<RunConfig>,
    tree: ProjectTree,
    docs: Documents,
    console: Console,
    sync: GradleSync,
    /// Подсказки Java (jdtls) для открытого проекта.
    lsp: Option<JavaLsp>,
    states: EditorStates,
    /// Перезапуск: дождаться остановки и запустить снова.
    rerun_pending: bool,
    /// Экран приветствия вместо рабочего интерфейса.
    welcome: bool,
    modal: Option<Modal>,
    modal_focus: bool,
    toast: Option<Toast>,
    cmds: Vec<Cmd>,
    gpu_info: String,
    window_title: String,
    was_focused: bool,
    logo: Option<(u32, egui::TextureHandle)>,
    mark: Option<(u32, egui::TextureHandle)>,
}

/// Раскладка без проекта.
fn global_layout_path() -> Option<PathBuf> {
    settings::config_dir().map(|d| d.join("layout.json"))
}

/// Стандартная раскладка для проектов, у которых своей ещё нет.
fn default_layout_path() -> Option<PathBuf> {
    settings::config_dir().map(|d| d.join("default-layout.json"))
}

fn project_layout_path(root: &Path) -> PathBuf {
    store::dir(root).join(store::WORKSPACE_FILE)
}

/// Стандартная раскладка, а если её нет — заводская.
fn default_layout() -> Layout {
    default_layout_path().and_then(|p| workspace::read_layout(&p)).map(|(l, _)| l).unwrap_or_else(Layout::factory)
}

impl MyceliumApp {
    pub fn new(cc: &eframe::CreationContext<'_>, settings: Settings, project_arg: Option<PathBuf>) -> Self {
        let ctx = &cc.egui_ctx;
        crate::fonts::install(ctx);
        crate::highlight::warm_up();
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

        let (layout, view) = global_layout_path().and_then(|p| workspace::read_layout(&p)).unwrap_or_else(|| (default_layout(), ViewState::default()));
        let mut app = Self {
            ctx: ctx.clone(),
            settings,
            pal,
            style_key: None,
            icon,
            dock_style,
            layout,
            view,
            project: None,
            configs: Vec::new(),
            tree: ProjectTree::default(),
            docs: Documents::default(),
            console: Console::default(),
            sync: GradleSync::default(),
            lsp: None,
            states: EditorStates::default(),
            rerun_pending: false,
            welcome: true,
            modal: None,
            modal_focus: false,
            toast: None,
            cmds: Vec::new(),
            gpu_info,
            window_title: String::new(),
            was_focused: true,
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
        self.toast = Some(Toast { text: text.into(), error, until: now + if error { 5.0 } else { 2.8 } });
    }

    /// Где сохраняется текущая раскладка: в проекте или в папке настроек.
    fn layout_path(&self) -> Option<PathBuf> {
        match &self.project {
            Some(p) => Some(project_layout_path(&p.root)),
            None => global_layout_path(),
        }
    }

    /// Сохраняет настройки, файлы и раскладку текущего проекта.
    fn persist(&mut self) {
        self.settings.save();
        for err in self.docs.save_all() {
            log::warn!("{err}");
        }
        if self.project.is_some() {
            self.view.expanded = self.tree.expanded_relative();
            self.view.show_hidden = self.tree.show_hidden;
            self.view.libs_open = self.tree.libs_open;
        }
        if let Some(path) = self.layout_path() {
            workspace::write_layout(&path, &self.layout, &self.view);
        }
    }

    fn open_project(&mut self, path: PathBuf, now: f64) {
        if self.project.as_ref().is_some_and(|p| std::path::absolute(&path).is_ok_and(|a| a == p.root)) {
            self.welcome = false;
            return;
        }
        let project = match Project::open(&path) {
            Ok(p) => p,
            Err(_) => {
                self.settings.recent_projects.retain(|p| p != &path);
                if self.settings.last_project.as_ref() == Some(&path) {
                    self.settings.last_project = None;
                }
                self.toast(t!("toast-project-missing", path = path.display().to_string()), true, now);
                return;
            }
        };
        // Прежний проект (или раскладка без проекта) сохраняется на своё место.
        self.persist();
        self.console.stop();
        self.docs.clear();

        let (layout, view) = workspace::read_layout(&project_layout_path(&project.root)).unwrap_or_else(|| (default_layout(), ViewState::default()));
        self.layout = layout;
        self.view = view;
        self.tree = ProjectTree::restore(&project.root, &self.view.expanded, self.view.show_hidden);
        self.tree.libs_open = self.view.libs_open;
        self.console = Console::default();
        self.states = EditorStates::default();
        self.configs = run::configs(&project.info, project.model.as_ref());
        self.sync.on_open(&project, now);
        // Подсказки Java: jdtls запускается сам, если установлен.
        self.lsp = (project.info.gradle && project.info.languages.contains(&mycelium_project::Language::Java))
            .then(|| JavaLsp::new(&self.ctx, &project.root, project.model.as_ref().and_then(|m| m.jdk()).map(|j| j.home.clone())));
        if !self.configs.iter().any(|c| Some(&c.id) == self.view.run_config.as_ref()) {
            self.view.run_config = self.configs.first().map(|c| c.id.clone());
        }
        self.settings.remember_project(&project.root);
        if project.created {
            self.toast(t!("toast-project-initialized"), false, now);
        }
        self.project = Some(project);
        self.welcome = false;
        // Сразу сохраняем раскладку: проект без неё получил стандартную.
        self.persist();
    }

    fn close_project(&mut self) {
        self.persist();
        self.console.stop();
        self.sync.reset();
        self.lsp = None;
        self.docs.clear();
        self.project = None;
        self.configs.clear();
        self.console = Console::default();
        self.states = EditorStates::default();
        self.settings.last_project = None;
        let (layout, view) = global_layout_path().and_then(|p| workspace::read_layout(&p)).unwrap_or_else(|| (default_layout(), ViewState::default()));
        self.layout = layout;
        self.view = view;
        self.welcome = true;
    }

    /// Открывает файл во вкладке «Код»: уже открытый — показывает, иначе занимает пустую
    /// вкладку «Код» или добавляется в область, где уже есть код.
    fn open_file(&mut self, path: PathBuf, line: Option<u32>, now: f64) {
        let Some(project) = &self.project else { return };
        let abs = if path.is_absolute() { path.clone() } else { project.root.join(&path) };
        if !abs.is_file() {
            self.toast(t!("toast-file-missing", path = abs.display().to_string()), true, now);
            return;
        }
        let rel = project.relative(&abs).unwrap_or_else(|| abs.clone());
        if let Some(line) = line {
            self.docs.open(&abs).jump = Some(line);
        }
        self.tree.reveal(&abs);
        self.place_tab(rel, None);
    }

    /// Открывает файл из jar (исходник Minecraft, ресурс библиотеки) только для чтения.
    fn open_jar(&mut self, jar: PathBuf, entry: String, line: Option<u32>) {
        if let Some(line) = line {
            self.docs.open_jar(&jar, &entry).jump = Some(line);
        }
        self.place_tab(jar, Some(entry));
    }

    /// Показывает вкладку файла: уже открытую — делает активной, иначе занимает пустую
    /// вкладку «Код» или добавляется в область, где уже есть код.
    fn place_tab(&mut self, file: PathBuf, entry: Option<String>) {
        // 1. Вкладка с этим файлом уже есть — показываем её.
        let active = self.layout.active;
        let mut found: Option<(SurfaceKey, mycelium_layout::AreaId)> = None;
        {
            let mut check = |key: SurfaceKey, s: &mut Surface<EditorTab>| {
                if found.is_some() {
                    return;
                }
                s.for_each_area_mut(|a| {
                    if found.is_none() {
                        if let Some(i) = a.tabs.iter().position(|t| t.shows(&file, entry.as_deref())) {
                            a.active = i;
                            found = Some((key, a.id));
                        }
                    }
                });
            };
            check(SurfaceKey::Main, &mut self.layout.workspaces[active].surface);
            for w in &mut self.layout.dock.windows {
                check(SurfaceKey::Window(w.id), &mut w.surface);
            }
        }
        if let Some((key, area)) = found {
            self.reveal_area(key, area);
            return;
        }

        // 2. Пустая вкладка «Код» или область с кодом в главном окне.
        let surface = &mut self.layout.workspaces[active].surface;
        let mut placed: Option<mycelium_layout::AreaId> = None;
        surface.for_each_area_mut(|a| {
            if placed.is_none() {
                if let Some(i) = a.tabs.iter().position(|t| t.kind == EditorKind::Code && t.file.is_none()) {
                    a.tabs[i].file = Some(file.clone());
                    a.tabs[i].entry = entry.clone();
                    a.active = i;
                    placed = Some(a.id);
                }
            }
        });
        let new_tab = |next: &mut u64| match &entry {
            Some(e) => EditorTab::jar(file.clone(), e.clone(), next),
            None => EditorTab::file(file.clone(), next),
        };
        if placed.is_none() {
            let next = &mut self.layout.next_tab_id;
            surface.for_each_area_mut(|a| {
                if placed.is_none() && a.tabs.iter().any(|t| t.kind == EditorKind::Code) {
                    a.tabs.push(new_tab(next));
                    a.active = a.tabs.len() - 1;
                    placed = Some(a.id);
                }
            });
        }
        match placed {
            Some(area) => self.reveal_area(SurfaceKey::Main, area),
            // 3. Кода нигде нет — в последнюю область, с которой работали.
            None => {
                let surface = &mut self.layout.workspaces[active].surface;
                if surface.maximized.is_some() && surface.maximized != self.layout.dock.focused(SurfaceKey::Main) {
                    surface.maximized = None;
                }
                let tab = new_tab(&mut self.layout.next_tab_id);
                self.layout.dock.open_tab(tab);
            }
        }
    }

    /// Делает область текущей и видимой: если развёрнута другая область, раскладка возвращается.
    fn reveal_area(&mut self, key: SurfaceKey, area: mycelium_layout::AreaId) {
        self.layout.dock.focus(key, area);
        let surface = match key {
            SurfaceKey::Main => Some(&mut self.layout.workspaces[self.layout.active].surface),
            SurfaceKey::Window(id) => self.layout.dock.windows.iter_mut().find(|w| w.id == id).map(|w| &mut w.surface),
        };
        if let Some(s) = surface {
            if s.maximized.is_some_and(|m| m != area) {
                s.maximized = None;
            }
        }
    }

    /// Исходник класса из стек-трейса: пакет → папка в исходниках мода.
    fn open_source(&mut self, class: &str, file: Option<&str>, line: Option<u32>, now: f64) {
        let Some(project) = &self.project else { return };
        let outer = class.split('$').next().unwrap_or(class);
        let (pkg, simple) = outer.rsplit_once('.').unwrap_or(("", outer));
        let names: Vec<String> = match file {
            Some(f) => vec![f.to_owned()],
            None => vec![format!("{simple}.java"), format!("{simple}.kt")],
        };
        let pkg_dir = pkg.replace('.', "/");
        let pkg_dir = &pkg_dir;
        let found = project
            .info
            .source_roots()
            .flat_map(|root| names.iter().map(move |n| project.root.join(root).join(pkg_dir).join(n)))
            .find(|p| p.is_file());
        // Не свой класс — ищем исходник в библиотеках (Minecraft, загрузчик, моды).
        let source = || {
            let libs = project.libraries.as_ref()?;
            let (lib, entry) = libs.find_class(class)?;
            libs.source_for(lib, &entry, file)
        };
        let source = if found.is_none() { source() } else { None };
        match (found, source) {
            (Some(path), _) => self.open_file(path, line, now),
            (None, Some((jar, entry))) => self.open_jar(jar, entry, line),
            (None, None) => self.toast(t!("toast-source-missing", class = class.to_owned()), true, now),
        }
    }

    /// Файл из дерева библиотек: у класса открывается исходник, остальное — как есть.
    fn open_library_entry(&mut self, lib: usize, entry: &str, now: f64) {
        let Some(libs) = self.project.as_ref().and_then(|p| p.libraries.clone()) else { return };
        let Some(l) = libs.libs.get(lib) else { return };
        if entry.ends_with(".class") {
            match libs.source_for(lib, entry, None) {
                Some((jar, path)) => self.open_jar(jar, path, None),
                None => {
                    let class = entry.trim_end_matches(".class").replace('/', ".");
                    self.toast(t!("toast-no-sources", class = class, jar = l.name.clone()), true, now);
                }
            }
        } else {
            self.open_jar(l.jar.clone(), entry.to_owned(), None);
        }
    }

    /// Есть ли в текущем пространстве или отдельных окнах редактор такого вида.
    fn has_editor(&self, kind: EditorKind) -> bool {
        self.layout.workspaces[self.layout.active]
            .surface
            .areas()
            .into_iter()
            .chain(self.layout.dock.windows.iter().flat_map(|w| w.surface.areas()))
            .any(|a| a.tabs.iter().any(|t| t.kind == kind))
    }

    fn selected_config(&self) -> Option<RunConfig> {
        self.configs.iter().find(|c| Some(&c.id) == self.view.run_config.as_ref()).or(self.configs.first()).cloned()
    }

    fn start_run(&mut self, now: f64) {
        let (Some(project), Some(config)) = (&self.project, self.selected_config()) else { return };
        if self.console.is_running() {
            return;
        }
        let root = project.root.clone();
        let rules = project.rules.clone();
        for err in self.docs.save_all() {
            self.toast(err, true, now);
        }
        self.console.start(&root, &config, rules, now);
        if !self.has_editor(EditorKind::Console) {
            let tab = EditorTab::new(EditorKind::Console, &mut self.layout.next_tab_id);
            self.layout.dock.open_tab(tab);
        }
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
                Cmd::CloseProject => self.close_project(),
                Cmd::ContinueWithoutProject => self.welcome = false,
                Cmd::OpenEditor(kind) => {
                    if !self.has_editor(kind) || matches!(kind, EditorKind::Code | EditorKind::ModMap | EditorKind::ModelPreview) {
                        let tab = EditorTab::new(kind, &mut self.layout.next_tab_id);
                        self.layout.dock.open_tab(tab);
                    } else {
                        self.show_editor(kind);
                    }
                }
                Cmd::OpenFile { path, line } => self.open_file(path, line, now),
                Cmd::OpenSource { class, file, line } => self.open_source(&class, file.as_deref(), line, now),
                Cmd::OpenLibraryEntry { lib, entry } => self.open_library_entry(lib, &entry, now),
                Cmd::Reveal(path) => reveal_in_explorer(&path),
                Cmd::Redetect => {
                    if let Some(p) = &mut self.project {
                        p.redetect();
                        self.configs = run::configs(&p.info, p.model.as_ref());
                    }
                }
                Cmd::SyncGradle => {
                    if let Some(p) = &self.project {
                        self.sync.start(p, now);
                    }
                }
                Cmd::InstallJdtls => {
                    if let Some(l) = &mut self.lsp {
                        l.install();
                    }
                }
                Cmd::RestartJdtls => {
                    if let Some(l) = &mut self.lsp {
                        l.restart();
                    }
                }
                Cmd::SaveAll => {
                    for err in self.docs.save_all() {
                        self.toast(err, true, now);
                    }
                }
                Cmd::Run => self.start_run(now),
                Cmd::Stop => self.console.stop(),
                Cmd::Rerun => {
                    self.console.stop();
                    self.rerun_pending = true;
                }
                Cmd::ClearConsole => self.console.clear(),
                Cmd::ClearProblems => self.console.problems.clear(),
                Cmd::SwitchWorkspace(i) => {
                    if i < self.layout.workspaces.len() {
                        self.layout.active = i;
                    }
                }
                Cmd::NewWorkspace(template) => {
                    let ws = match template {
                        Some(b) => Workspace { builtin: Some(b), name: None, surface: workspace::builtin_surface(b, &mut self.layout.ids()) },
                        None => self.copy_workspace(self.layout.active),
                    };
                    self.layout.workspaces.push(ws);
                    self.layout.active = self.layout.workspaces.len() - 1;
                }
                Cmd::DuplicateWorkspace(i) => {
                    if i < self.layout.workspaces.len() {
                        let ws = self.copy_workspace(i);
                        self.layout.workspaces.insert(i + 1, ws);
                        self.layout.active = i + 1;
                    }
                }
                Cmd::RenameWorkspace(i) => {
                    if let Some(ws) = self.layout.workspaces.get(i) {
                        self.modal = Some(Modal::Rename { index: i, name: ws.title() });
                        self.modal_focus = true;
                    }
                }
                Cmd::ResetWorkspace(i) => {
                    if let Some(b) = self.layout.workspaces.get(i).and_then(|w| w.builtin) {
                        let surface = workspace::builtin_surface(b, &mut self.layout.ids());
                        self.layout.workspaces[i].surface = surface;
                    }
                }
                Cmd::DeleteWorkspace(i) => {
                    let ws = &mut self.layout.workspaces;
                    if ws.len() > 1 && i < ws.len() {
                        ws.remove(i);
                        if self.layout.active > i || self.layout.active >= ws.len() {
                            self.layout.active = self.layout.active.saturating_sub(1);
                        }
                    }
                }
                Cmd::ResetAllWorkspaces => {
                    self.layout.workspaces = workspace::default_workspaces(&mut self.layout.ids());
                    self.layout.active = 0;
                }
                Cmd::SaveDefaultLayout => match (self.layout.without_files(), default_layout_path()) {
                    (Some(copy), Some(path)) => {
                        workspace::write_layout(&path, &copy, &ViewState::default());
                        self.toast(t!("toast-default-layout-saved"), false, now);
                    }
                    _ => self.toast(t!("toast-default-layout-failed"), true, now),
                },
                Cmd::ApplyDefaultLayout => {
                    self.layout = default_layout();
                    self.toast(t!("toast-default-layout-applied"), false, now);
                }
                Cmd::ResetDefaultLayout => {
                    if let Some(path) = default_layout_path() {
                        let _ = std::fs::remove_file(path);
                    }
                    self.toast(t!("toast-default-layout-reset"), false, now);
                }
                Cmd::DetachFocused => self.layout.dock.detach_focused(&self.layout.workspaces[self.layout.active].surface),
                Cmd::ReturnAllWindows => self.layout.dock.return_all(),
                Cmd::ToggleMaximize => self.layout.dock.toggle_maximize_focused(&self.layout.workspaces[self.layout.active].surface),
                Cmd::Settings => self.modal = Some(Modal::Settings),
                Cmd::About => self.modal = Some(Modal::About),
                Cmd::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            }
        }
    }

    /// Показывает уже открытый редактор: делает его вкладку активной.
    fn show_editor(&mut self, kind: EditorKind) {
        let active = self.layout.active;
        let mut found = None;
        self.layout.workspaces[active].surface.for_each_area_mut(|a| {
            if found.is_none() {
                if let Some(i) = a.tabs.iter().position(|t| t.kind == kind) {
                    a.active = i;
                    found = Some((SurfaceKey::Main, a.id));
                }
            }
        });
        for w in &mut self.layout.dock.windows {
            w.surface.for_each_area_mut(|a| {
                if found.is_none() {
                    if let Some(i) = a.tabs.iter().position(|t| t.kind == kind) {
                        a.active = i;
                        found = Some((SurfaceKey::Window(w.id), a.id));
                    }
                }
            });
        }
        if let Some((key, area)) = found {
            self.reveal_area(key, area);
        }
    }

    /// Копия пространства с новыми номерами областей и вкладок.
    fn copy_workspace(&mut self, i: usize) -> Workspace {
        let mut ws = self.layout.workspaces[i].clone();
        ws.name = Some(t!("workspace-copy-name", name = ws.title()));
        let dock = &mut self.layout.dock;
        ws.surface.reassign_ids(|| dock.new_area_id());
        let next = &mut self.layout.next_tab_id;
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
        if ctx.input_mut(|i| i.consume_shortcut(&SAVE_SHORTCUT)) {
            self.cmds.push(Cmd::SaveAll);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&RUN_SHORTCUT)) {
            self.cmds.push(Cmd::Run);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&STOP_SHORTCUT)) {
            self.cmds.push(Cmd::Stop);
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
            Some(p) => format!("{} — {APP_TITLE}", p.name()),
            None => APP_TITLE.to_owned(),
        };
        if title != self.window_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }
    }

    /// Фоновая работа кадра: вывод запуска, автосохранение, файлы, изменённые снаружи.
    fn background(&mut self, ctx: &egui::Context, now: f64) {
        if self.console.poll(now) || self.console.is_running() {
            ctx.request_repaint_after_secs(0.1);
        }
        self.apply_sync(now);
        if self.sync.is_busy() {
            ctx.request_repaint_after_secs(0.2);
        }
        self.poll_lsp(ctx, now);
        if self.rerun_pending && !self.console.is_running() {
            self.rerun_pending = false;
            self.start_run(now);
        }
        let (errors, waiting) = self.docs.autosave(now);
        for err in errors {
            self.toast(err, true, now);
        }
        if waiting {
            ctx.request_repaint_after_secs(0.3);
        }
        // Вернулись в окно — файлы могли поменять в другой программе.
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
        if focused && !self.was_focused {
            self.docs.reload_changed();
            self.tree.refresh();
        }
        if !focused && self.was_focused {
            for err in self.docs.save_all() {
                self.toast(err, true, now);
            }
        }
        self.was_focused = focused;

        // Документы без вкладок больше не нужны.
        if let Some(project) = &self.project {
            let open: HashSet<PathBuf> =
                self.layout.surfaces().flat_map(|s| s.areas().into_iter().flat_map(|a| a.tabs.iter().filter_map(|t| t.doc_key(&project.root)))).collect();
            self.docs.retain_open(&open);
        }
    }

    /// Сообщения jdtls и переходы к определению.
    fn poll_lsp(&mut self, ctx: &egui::Context, now: f64) {
        let Some(lsp) = &mut self.lsp else { return };
        if lsp.poll(&self.docs) {
            ctx.request_repaint();
        }
        if !matches!(lsp.status, LspStatus::Ready | LspStatus::NotInstalled | LspStatus::NoJdk(_) | LspStatus::Failed(_)) {
            ctx.request_repaint_after_secs(0.25);
        }
        let events: Vec<LspEvent> = lsp.events.drain(..).collect();
        for event in events {
            match event {
                LspEvent::Open { path, line } => self.open_file(path, Some(line), now),
                LspEvent::OpenClass { class, line } => self.open_source(&class, None, Some(line), now),
                LspEvent::NotFound => self.toast(t!("toast-definition-missing"), false, now),
            }
        }
    }

    /// Результаты синхронизации с Gradle: модель (конфигурации запуска) и библиотеки.
    fn apply_sync(&mut self, now: f64) {
        for result in self.sync.poll() {
            let Some(project) = &mut self.project else { continue };
            match result {
                SyncResult::Model(model) => {
                    project.model = Some(model);
                    self.configs = run::configs(&project.info, project.model.as_ref());
                    if !self.configs.iter().any(|c| Some(&c.id) == self.view.run_config.as_ref()) {
                        self.view.run_config = self.configs.first().map(|c| c.id.clone());
                    }
                }
                SyncResult::Libraries(libs) => project.libraries = Some(libs),
                SyncResult::Failed { message, output } => {
                    for line in &output {
                        log::warn!("gradle: {line}");
                    }
                    if message != "cancelled" {
                        self.toast(t!("toast-sync-failed", error = message), true, now);
                    }
                }
            }
        }
    }

    fn dock_ui(&mut self, ui: &mut Ui, ctx: &egui::Context, now: f64) {
        let active = self.layout.active;
        let ws = &mut self.layout.workspaces[active];
        let mut viewer = Editors {
            pal: &self.pal,
            project: self.project.as_ref(),
            tree: &mut self.tree,
            recent: &self.settings.recent_projects,
            next_tab_id: &mut self.layout.next_tab_id,
            cmds: &mut self.cmds,
            docs: &mut self.docs,
            console: &self.console,
            sync: &self.sync.state,
            lsp: self.lsp.as_mut(),
            configs: &self.configs,
            run_config: &mut self.view.run_config,
            states: &mut self.states,
            font_size: self.settings.font_size,
            now,
        };
        let dock = &mut self.layout.dock;
        let style = &self.dock_style;
        egui::CentralPanel::default().frame(Frame::new().fill(self.pal.bg_deep)).show(ui, |ui| {
            dock.show_main(ui, &mut ws.surface, &mut viewer, style);
        });
        dock.show_windows(ctx, &mut viewer, style);
        dock.finish(ctx, &mut ws.surface, &mut viewer);
    }
}

impl eframe::App for MyceliumApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let now = Self::now(&ctx);
        self.apply_style(&ctx);
        self.handle_keys(&ctx);
        self.settings.zoom = ctx.zoom_factor();
        if self.layout.active >= self.layout.workspaces.len() {
            self.layout.active = 0;
        }
        self.background(&ctx, now);

        if self.welcome {
            self.welcome_ui(ui);
        } else {
            self.top_bar(ui);
            self.status_bar(ui);
            self.dock_ui(ui, &ctx, now);
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
        self.console.stop();
        // jdtls завершается по-хорошему, чтобы сохранить свою рабочую папку.
        self.lsp = None;
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.pal.bg_deep.to_normalized_gamma_f32()
    }
}

fn reveal_in_explorer(path: &Path) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer").arg(format!("/select,{}", path.display())).spawn();
    }
    #[cfg(not(windows))]
    {
        let dir = if path.is_dir() { path.to_path_buf() } else { path.parent().map(Path::to_path_buf).unwrap_or_default() };
        let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
        let _ = std::process::Command::new(opener).arg(dir).spawn();
    }
}
