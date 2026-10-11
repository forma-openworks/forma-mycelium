//! Главное окно Forma Mycelium: проект, раскладка, редакторы, запуск и команды.
//!
//! Раскладка интерфейса принадлежит проекту и хранится в его `.mycelium/workspace.json`.
//! Проект без своей раскладки получает стандартную (её задаёт «Сохранить раскладку как
//! стандартную»), а если и её нет — заводскую. Без проекта используется отдельная раскладка
//! в папке настроек.

mod caption;
mod chrome;
mod look;
mod run_configs;
mod settings_dialog;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::sync::Arc;

use eframe::egui::{self, Frame, Ui};
use forma_actions::{Context as ActionContext, Registry, Shortcut};
use forma_i18n::t;
use forma_layout::{DockStyle, Surface, SurfaceKey};
use forma_settings::Store;
use forma_ui::appearance::ZOOM_RANGE;
use forma_ui::find_action::FindAction;
use mycelium_project::{Project, store};

use crate::editors::{Documents, EditorKind, EditorStates, EditorTab, Editors, ProjectTree};
use crate::run::{self, Console, RunConfig, RunsFile};
use run_configs::RunConfigsEditor;
use settings_dialog::{ExportDialog, ImportDialog, SettingsDialog};
use crate::actions;
use crate::colors::{BUILTIN_THEMES, DEFAULT_THEME, ModColors};
use crate::settings::{self, Settings};
use crate::discord::{self, ModLinks, PresenceInput, Presence};
use crate::lsp::{JavaLsp, LspEvent, LspStatus};
use crate::sync::{GradleSync, SyncResult};
use forma_ui::fonts::Choice;
use forma_ui::theme::{self, Palette, Theme, Themes};
use crate::workspace::{self, Builtin, Layout, ViewState, Workspace};

pub const APP_TITLE: &str = "Forma Mycelium";

/// Иконка окна и панели задач. Там она маленькая — поэтому упрощённый рисунок.
pub fn app_icon() -> egui::IconData {
    let size = 256;
    egui::IconData { rgba: crate::icon::rgba(crate::icon::Logo::Simple, size), width: size, height: size }
}

/// Действия, которые копятся за кадр и выполняются в его конце.
pub enum Cmd {
    OpenProjectDialog,
    OpenProject(PathBuf),
    CloseProject,
    ContinueWithoutProject,
    OpenEditor(EditorKind),
    /// Показать проблему (по подписи) в окне исключений.
    ShowProblem(String),
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
    /// Поставить плагин из каталога.
    InstallPlugin(String),
    /// Поставить плагин из `.zip` (спросить файл).
    InstallPluginFile,
    UninstallPlugin(String),
    /// Открыть папку плагинов.
    RevealPlugins,
    /// Перезапустить jdtls (а если его нет — поставить).
    RestartJdtls,
    SaveAll,
    /// Запустить выбранную конфигурацию (уже идущую — перезапустить).
    Run,
    /// Выбрать конфигурацию и запустить.
    RunConfig(String),
    /// Остановить выбранную конфигурацию.
    Stop,
    StopAll,
    StopSession(u64),
    RestartSession(u64),
    /// Убрать вкладку законченного запуска.
    CloseSession(u64),
    ClearSession(u64),
    EditRunConfigs,
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
    /// Открыть настройки на странице.
    OpenSettingsPage(String),
    ExportSettings,
    ImportSettings,
    FindAction,
    /// Выполнить действие по id (из «Найти действие» или отдельного окна).
    Action(String),
    ZoomIn,
    ZoomOut,
    ZoomReset,
    /// Быстрая смена темы с предпросмотром.
    QuickSwitchTheme,
    /// Ctrl+колёсико в редакторе или консоли: размер шрифта на `step` пикселей.
    FontZoom { console: bool, step: f32 },
    About,
    Quit,
}

/// Команда, которую выполняет общее действие. У действий редактора команды нет:
/// их сочетания ловит сам редактор.
pub fn action_cmd(id: &str) -> Option<Cmd> {
    use actions::*;
    Some(match id {
        OPEN_PROJECT => Cmd::OpenProjectDialog,
        CLOSE_PROJECT => Cmd::CloseProject,
        SAVE_ALL => Cmd::SaveAll,
        SETTINGS => Cmd::Settings,
        EXPORT_SETTINGS => Cmd::ExportSettings,
        IMPORT_SETTINGS => Cmd::ImportSettings,
        QUIT => Cmd::Quit,
        SYNC_GRADLE => Cmd::SyncGradle,
        RUN => Cmd::Run,
        STOP => Cmd::Stop,
        STOP_ALL => Cmd::StopAll,
        EDIT_RUN_CONFIGS => Cmd::EditRunConfigs,
        ZOOM_IN => Cmd::ZoomIn,
        ZOOM_OUT => Cmd::ZoomOut,
        ZOOM_RESET => Cmd::ZoomReset,
        QUICK_THEME => Cmd::QuickSwitchTheme,
        SHOW_PROJECT => Cmd::OpenEditor(EditorKind::Project),
        SHOW_CONSOLE => Cmd::OpenEditor(EditorKind::Console),
        SHOW_EXCEPTIONS => Cmd::OpenEditor(EditorKind::Exceptions),
        MAXIMIZE => Cmd::ToggleMaximize,
        DETACH => Cmd::DetachFocused,
        RETURN_WINDOWS => Cmd::ReturnAllWindows,
        FIND_ACTION => Cmd::FindAction,
        ABOUT => Cmd::About,
        _ => return None,
    })
}

enum Modal {
    Settings(Box<SettingsDialog>),
    FindAction(FindAction),
    ThemeSwitcher(look::ThemeSwitcher),
    Export(ExportDialog),
    Import(Box<ImportDialog>),
    About,
    Rename { index: usize, name: String },
    RunConfigs(Box<RunConfigsEditor>),
}

struct Toast {
    text: String,
    error: bool,
    until: f64,
}

/// От чего зависит оформление: при изменении тема вычисляется заново.
#[derive(Clone, Debug, PartialEq)]
struct StyleKey {
    theme: String,
    themes_rev: u64,
    ui_size: u32,
    max_keys: Vec<Shortcut>,
}

pub struct MyceliumApp {
    /// Контекст egui — фоновые задачи будят им интерфейс.
    ctx: egui::Context,
    settings: Settings,
    /// Файлы компонентов настроек (`options/*.json`).
    store: Store,
    /// Все действия программы — для раскладки клавиш, меню и «Найти действие».
    actions: Registry,
    /// Последняя открытая страница настроек.
    settings_page: String,
    /// Встроенные и свои темы.
    themes: Themes,
    /// Номер изменения своих тем: растёт, когда файл темы поменяли на диске.
    themes_rev: u64,
    /// Текущая тема и её цвета интерфейса.
    theme: Theme,
    pal: Palette,
    /// Номер темы для кеша подсветки кода.
    theme_gen: u64,
    style_key: Option<StyleKey>,
    /// Установленные шрифты.
    fonts_key: Option<Choice>,
    /// Фоновая картинка, загруженная в видеопамять.
    backdrop: look::Backdrop,
    icon: Arc<egui::IconData>,
    dock_style: DockStyle,
    layout: Layout,
    /// Что проект помнит об интерфейсе кроме раскладки (дерево, конфигурация запуска).
    view: ViewState,
    project: Option<Project>,
    configs: Vec<RunConfig>,
    /// Свои конфигурации запуска и правки найденных (`.mycelium/runs.json`).
    runs: RunsFile,
    tree: ProjectTree,
    docs: Documents,
    console: Console,
    sync: GradleSync,
    /// Подсказки Java (jdtls) для открытого проекта.
    lsp: Option<JavaLsp>,
    /// Моды запуска открытого проекта (Hyphae): что попадёт в игру и чьи у них пакеты.
    run_mods: Option<Arc<mycelium_hyphae::RunMods>>,
    /// Установленные плагины и идущие установки.
    plugins: forma_plugins::Plugins,
    states: EditorStates,
    /// Экран приветствия вместо рабочего интерфейса.
    welcome: bool,
    modal: Option<Modal>,
    modal_focus: bool,
    toast: Option<Toast>,
    cmds: Vec<Cmd>,
    gpu_info: String,
    window_title: String,
    was_focused: bool,
    /// С какого момента окно не в фокусе (для статуса «перерыв» в Discord).
    unfocused_since: Option<f64>,
    presence: Presence,
    /// Иконка и репозиторий мода на GitHub (ищутся в фоне при открытии проекта).
    links: Option<ModLinks>,
    links_rx: Option<Receiver<ModLinks>>,
    /// Когда открыт проект (или запущена программа) — начало таймера в Discord.
    opened_unix: u64,
    logo: Option<(u32, egui::TextureHandle)>,
    mark: Option<(u32, egui::TextureHandle)>,
    /// Высота верхней строки на прошлом кадре — под неё подстраиваются кнопки окна.
    caption_h: f32,
}

/// Какие шрифты ставить: свои из настроек или шрифты темы.
fn font_choice(f: &forma_ui::fonts::Fonts, theme: &forma_ui::theme::FontsSpec) -> Choice {
    Choice { ui: f.ui.effective(theme.ui.as_deref()), editor: f.editor.effective(theme.editor.as_deref()), console: f.console.effective(theme.console.as_deref()) }
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
    pub fn new(cc: &eframe::CreationContext<'_>, settings: Settings, store: Store, project_arg: Option<PathBuf>) -> Self {
        let ctx = &cc.egui_ctx;
        // Системной рамки у окна нет — шапку рисует программа (см. `caption`).
        caption::round_corners(cc);
        crate::highlight::warm_up();
        // Список шрифтов системы нужен странице «Шрифты» и темам со своими шрифтами.
        forma_ui::fonts::warm_up();
        // Масштаб и выход — действия из раскладки клавиш, а не встроенные сочетания egui:
        // иначе случайное Ctrl+Q молча закрывало бы программу.
        ctx.options_mut(|o| {
            o.zoom_with_keyboard = false;
            o.quit_shortcuts.clear();
        });
        ctx.set_zoom_factor(settings.appearance.zoom.clamp(*ZOOM_RANGE.start(), *ZOOM_RANGE.end()));
        let plugins = crate::plugins::open(&settings.appearance.theme);
        let mut themes = Themes::new(BUILTIN_THEMES, settings::user_themes_dir());
        themes.set_plugin_dirs(crate::plugins::theme_dirs(&plugins, &settings.plugins));
        let theme = themes.resolve(&settings.appearance.theme, DEFAULT_THEME);
        let pal = theme.pal.clone();
        // Шрифты нужны уже в первом кадре: egui применяет новые шрифты только со следующего.
        let fonts_choice = font_choice(&settings.fonts, &theme.fonts);
        forma_ui::fonts::install(ctx, &fonts_choice);
        let icon = Arc::new(app_icon());
        let actions = actions::registry();
        let dock_style = pal.dock_style(APP_TITLE, icon.clone(), Vec::new(), String::new());
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
            store,
            actions,
            settings_page: forma_ui::appearance::PAGE.to_owned(),
            themes,
            themes_rev: 0,
            theme,
            pal,
            theme_gen: 0,
            style_key: None,
            fonts_key: Some(fonts_choice),
            backdrop: look::Backdrop::default(),
            icon,
            dock_style,
            layout,
            view,
            project: None,
            configs: Vec::new(),
            runs: RunsFile::default(),
            tree: ProjectTree::default(),
            docs: Documents::default(),
            console: Console::default(),
            sync: GradleSync::default(),
            lsp: None,
            run_mods: None,
            plugins,
            states: EditorStates::default(),
            welcome: true,
            modal: None,
            modal_focus: false,
            toast: None,
            cmds: Vec::new(),
            gpu_info,
            window_title: String::new(),
            was_focused: true,
            unfocused_since: None,
            presence: Presence::default(),
            links: None,
            links_rx: None,
            opened_unix: run::unix_now(),
            logo: None,
            mark: None,
            caption_h: caption::WELCOME_H,
        };
        // Как в IDEA: при запуске снова открывается последний проект.
        let path = project_arg.or_else(|| app.settings.recent.last.clone().filter(|p| p.is_dir()));
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
        self.settings.save(&mut self.store);
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
                self.settings.recent.projects.retain(|p| p != &path);
                if self.settings.recent.last.as_ref() == Some(&path) {
                    self.settings.recent.last = None;
                }
                self.toast(t!("toast-project-missing", path = path.display().to_string()), true, now);
                return;
            }
        };
        // Прежний проект (или раскладка без проекта) сохраняется на своё место.
        self.persist();
        self.console.stop_all();
        self.docs.clear();

        let (layout, view) = workspace::read_layout(&project_layout_path(&project.root)).unwrap_or_else(|| (default_layout(), ViewState::default()));
        self.layout = layout;
        self.view = view;
        self.tree = ProjectTree::restore(&project.root, &self.view.expanded, self.view.show_hidden);
        self.tree.libs_open = self.view.libs_open;
        self.console = Console::default();
        self.states = EditorStates::default();
        self.runs = RunsFile::load(&project.root);
        self.opened_unix = run::unix_now();
        // Иконку и репозиторий мода ищем в фоне: это git и сеть.
        self.links = None;
        let (tx, rx) = std::sync::mpsc::channel();
        let (root, icon) = (project.root.clone(), project.info.icon.clone());
        std::thread::spawn(move || {
            let _ = tx.send(discord::resolve_links(&root, icon.as_deref()));
        });
        self.links_rx = Some(rx);
        self.configs = run::configs(&project.info, project.model.as_ref(), &self.runs);
        self.run_mods = None;
        crate::mods_view::set(None, Default::default());
        crate::mods_view::set_rules(&project.rules);
        self.sync.on_open(&project, now);
        if project.model.is_none() {
            // Модели ещё нет — моды запуска пока только из `run/mods`.
            self.sync.refresh_mods(&project);
        }
        // Подсказки Java: jdtls запускается сам, если установлен.
        self.lsp = (project.info.gradle && project.info.languages.contains(&mycelium_project::Language::Java))
            .then(|| {
                let jdk = project.model.as_ref().and_then(|m| m.jdk()).map(|j| j.home.clone());
                JavaLsp::new(&self.ctx, &project.root, jdk, crate::plugins::active_dir(&self.plugins, &self.settings.plugins, crate::plugins::JDTLS))
            });
        if !self.configs.iter().any(|c| Some(&c.id) == self.view.run_config.as_ref()) {
            self.view.run_config = self.configs.first().map(|c| c.id.clone());
        }
        self.settings.recent.remember(&project.root);
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
        self.console.stop_all();
        self.sync.reset();
        self.lsp = None;
        self.run_mods = None;
        crate::mods_view::set(None, Default::default());
        crate::mods_view::set_rules(&Default::default());
        self.docs.clear();
        self.project = None;
        self.configs.clear();
        self.runs = RunsFile::default();
        self.links = None;
        self.links_rx = None;
        self.opened_unix = run::unix_now();
        self.console = Console::default();
        self.states = EditorStates::default();
        self.settings.recent.last = None;
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
        let mut found: Option<(SurfaceKey, forma_layout::AreaId)> = None;
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
        let mut placed: Option<forma_layout::AreaId> = None;
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
    fn reveal_area(&mut self, key: SurfaceKey, area: forma_layout::AreaId) {
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

    /// Пересчитывает конфигурации запуска (модель, правки) и следит, чтобы выбранная осталась.
    fn refresh_configs(&mut self) {
        let Some(p) = &self.project else { return };
        self.configs = run::configs(&p.info, p.model.as_ref(), &self.runs);
        if !self.configs.iter().any(|c| Some(&c.id) == self.view.run_config.as_ref()) {
            self.view.run_config = self.configs.first().map(|c| c.id.clone());
        }
    }

    /// Запускает выбранную конфигурацию; уже идущая перезапускается, остальные запуски не трогаются.
    fn start_run(&mut self, now: f64) {
        let (Some(project), Some(config)) = (&self.project, self.selected_config()) else { return };
        let root = project.root.clone();
        let rules = project.rules.clone();
        // В `run/mods` могли положить новый jar: моды запуска перечитываются в фоне,
        // и идущий запуск получит их, как только они будут готовы.
        self.sync.refresh_mods(project);
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
        // Команда может породить новую (`Action` → команда действия), поэтому — до пустой очереди.
        while !self.cmds.is_empty() {
            self.process_batch(ctx, now);
        }
    }

    fn process_batch(&mut self, ctx: &egui::Context, now: f64) {
        for cmd in std::mem::take(&mut self.cmds) {
            match cmd {
                Cmd::OpenProjectDialog => {
                    let mut dialog = rfd::FileDialog::new().set_title(t!("dialog-open-project"));
                    if let Some(dir) = self.settings.recent.projects.first().and_then(|p| p.parent()) {
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
                        if let Some(run) = &self.run_mods {
                            p.rules = p.rules.with_mods(run.map());
                        }
                        crate::mods_view::set_rules(&p.rules);
                    }
                    self.refresh_configs();
                }
                Cmd::SyncGradle => {
                    if let Some(p) = &self.project {
                        self.sync.start(p, now);
                    }
                }
                Cmd::InstallPlugin(id) => {
                    if let Some(entry) = crate::plugins::entry(&id) {
                        self.plugins.install(&entry);
                    }
                }
                Cmd::InstallPluginFile => {
                    let file = rfd::FileDialog::new().set_title(t!("dialog-install-plugin")).add_filter("zip", &["zip"]).pick_file();
                    if let Some(file) = file {
                        match self.plugins.install_file(&file) {
                            Ok(id) => {
                                let name = self.plugins.get(&id).map(|p| p.manifest.title().to_owned()).unwrap_or(id);
                                self.toast(t!("toast-plugin-installed", name = name), false, now);
                            }
                            Err(e) => self.toast(t!("toast-plugin-failed", error = e), true, now),
                        }
                    }
                }
                Cmd::UninstallPlugin(id) => {
                    // Выбранная тема из удаляемого плагина пропадёт — возвращаем тему по умолчанию.
                    if let Some(dir) = self.plugins.get(&id).map(|p| p.dir.clone())
                        && self.themes.get(&self.settings.appearance.theme).is_some_and(|t| matches!(&t.origin, theme::Origin::Plugin(p) if p.starts_with(&dir)))
                    {
                        self.settings.appearance.theme = String::new();
                    }
                    // Сначала отпускаем файлы плагина: jdtls останавливается.
                    if id == crate::plugins::JDTLS {
                        if let Some(l) = &mut self.lsp {
                            l.set_server(None);
                        }
                    }
                    if let Err(e) = self.plugins.uninstall(&id) {
                        self.toast(t!("toast-plugin-failed", error = e), true, now);
                    }
                }
                Cmd::RevealPlugins => {
                    if let Some(root) = self.plugins.root() {
                        let _ = std::fs::create_dir_all(root);
                        open_folder(root);
                    }
                }
                Cmd::RestartJdtls => match &mut self.lsp {
                    Some(l) if l.has_server() => l.restart(),
                    _ => self.cmds.push(Cmd::InstallPlugin(crate::plugins::JDTLS.into())),
                },
                Cmd::SaveAll => {
                    for err in self.docs.save_all() {
                        self.toast(err, true, now);
                    }
                }
                Cmd::Run => self.start_run(now),
                Cmd::RunConfig(id) => {
                    self.view.run_config = Some(id);
                    self.start_run(now);
                }
                Cmd::Stop => {
                    if let Some(id) = self.selected_config().and_then(|c| self.console.session_of(&c.id).map(|s| s.id)) {
                        self.console.stop(id);
                    }
                }
                Cmd::StopAll => self.console.stop_all(),
                Cmd::StopSession(id) => self.console.stop(id),
                Cmd::RestartSession(id) => {
                    let rules = self.project.as_ref().map(|p| p.rules.clone()).unwrap_or_default();
                    self.console.restart(id, rules, now);
                }
                Cmd::CloseSession(id) => self.console.close(id),
                Cmd::ClearSession(id) => self.console.clear(id),
                Cmd::EditRunConfigs => {
                    if let Some(p) = &self.project {
                        let detected = run::detected_configs(&p.info, p.model.as_ref());
                        let tasks = gradle_tasks(p);
                        let editor = RunConfigsEditor::new(detected, &self.runs, tasks, self.view.run_config.as_deref());
                        self.modal = Some(Modal::RunConfigs(Box::new(editor)));
                    }
                }
                Cmd::ClearProblems => self.console.problems.clear(),
                Cmd::ShowProblem(signature) => {
                    self.states.select_problem(&signature);
                    self.cmds.push(Cmd::OpenEditor(EditorKind::Exceptions));
                }
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
                Cmd::Settings => self.open_settings(None),
                Cmd::OpenSettingsPage(page) => self.open_settings(Some(page)),
                Cmd::ExportSettings => self.modal = Some(Modal::Export(ExportDialog::new())),
                Cmd::ImportSettings => self.import_settings(now),
                Cmd::FindAction => self.modal = Some(Modal::FindAction(FindAction::default())),
                Cmd::Action(id) => {
                    if let Some(cmd) = action_cmd(&id) {
                        self.cmds.push(cmd);
                    }
                }
                Cmd::ZoomIn => self.set_zoom(ctx, self.settings.appearance.zoom + 0.1),
                Cmd::ZoomOut => self.set_zoom(ctx, self.settings.appearance.zoom - 0.1),
                Cmd::ZoomReset => self.set_zoom(ctx, 1.0),
                Cmd::QuickSwitchTheme => self.modal = Some(Modal::ThemeSwitcher(look::ThemeSwitcher::new(&self.settings.appearance.theme, &self.themes))),
                Cmd::FontZoom { console, step } => {
                    let slot = if console { &mut self.settings.fonts.console } else { &mut self.settings.fonts.editor };
                    // Размер — с шагом в полпикселя, как у ползунка в настройках.
                    let size = ((slot.size + step) * 2.0).round() / 2.0;
                    slot.size = size.clamp(*forma_ui::fonts::SIZE_RANGE.start(), *forma_ui::fonts::SIZE_RANGE.end());
                }
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

    /// Общие сочетания клавиш главного окна. Пока открыт диалог, они не работают,
    /// как в JetBrains: иначе, например, запись сочетания в настройках запускала бы его.
    fn handle_keys(&mut self, ctx: &egui::Context) {
        if self.modal.is_some() {
            return;
        }
        let fired = ctx.input_mut(|i| forma_actions::triggered(i, &self.actions, &self.settings.keymap, ActionContext::Global));
        self.cmds.extend(fired.iter().filter_map(|id| action_cmd(id)));
    }

    fn set_zoom(&mut self, ctx: &egui::Context, zoom: f32) {
        let zoom = ((zoom * 10.0).round() / 10.0).clamp(*ZOOM_RANGE.start(), *ZOOM_RANGE.end());
        self.settings.appearance.zoom = zoom;
        ctx.set_zoom_factor(zoom);
    }

    /// Текст сочетания действия для меню.
    pub(super) fn keys(&self, id: &str) -> String {
        self.settings.keymap.text(&self.actions, id)
    }

    /// Тема, цвета, шрифты: пересчитываются, когда меняется что-то, от чего они зависят
    /// (в том числе файл своей темы на диске).
    fn apply_style(&mut self, ctx: &egui::Context) {
        if self.themes.poll(Self::now(ctx)) {
            self.themes_rev += 1;
        }
        let a = &self.settings.appearance;
        let key = StyleKey {
            theme: a.theme.clone(),
            themes_rev: self.themes_rev,
            ui_size: self.settings.fonts.ui.size.to_bits(),
            max_keys: self.settings.keymap.shortcuts(&self.actions, actions::MAXIMIZE).to_vec(),
        };
        if self.style_key.as_ref() != Some(&key) {
            self.theme = self.themes.resolve(&key.theme, DEFAULT_THEME);
            self.pal = self.theme.pal.clone();
            self.theme_gen += 1;
            crate::highlight::set_theme(&self.theme, self.theme_gen);
            crate::colors::set(self.theme.group(ModColors::TOKENS, ModColors::resolve));
            forma_ui::icons::set_theme(&self.theme.icons, &self.theme.icon_colors);
            let text = key.max_keys.first().map(ToString::to_string).unwrap_or_default();
            self.dock_style = self.pal.dock_style(APP_TITLE, self.icon.clone(), key.max_keys.iter().map(Shortcut::to_egui).collect(), text);
            theme::apply(ctx, &self.pal, self.settings.fonts.ui.size);
            self.style_key = Some(key);
        }
        let choice = font_choice(&self.settings.fonts, &self.theme.fonts);
        if self.fonts_key.as_ref() != Some(&choice) {
            forma_ui::fonts::install(ctx, &choice);
            self.fonts_key = Some(choice);
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
        let rules = self.project.as_ref().map(|p| p.rules.clone()).unwrap_or_default();
        if self.console.poll(&rules, now) || self.console.is_running() {
            ctx.request_repaint_after_secs(0.1);
        }
        self.apply_sync(now);
        if self.sync.is_busy() {
            ctx.request_repaint_after_secs(0.2);
        }
        self.poll_plugins(ctx, now);
        self.poll_lsp(ctx, now);
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
        self.unfocused_since = if focused { None } else { self.unfocused_since.or(Some(now)) };
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

    /// Активность в Discord: мод, что сейчас делается, таймер, значок загрузчика.
    fn update_presence(&mut self, now: f64) {
        if let Some(rx) = &self.links_rx {
            if let Ok(links) = rx.try_recv() {
                self.links = Some(links);
                self.links_rx = None;
            }
        }
        if !self.settings.discord.enabled {
            self.presence.set(None);
            return;
        }
        let running: Vec<_> = self.console.running().map(|s| (s.config.kind, s.config.label())).collect();
        // Упал последний законченный запуск, и после него ничего не запускали.
        let crashed = if running.is_empty() {
            self.console.sessions.iter().max_by_key(|s| s.started_unix).filter(|s| s.crashed).map(|s| s.config.kind)
        } else {
            None
        };
        let started_unix = self.console.running().map(|s| s.started_unix).min().unwrap_or(self.opened_unix);
        // Перерыв: окно давно не в фокусе и ничего не запущено.
        let away = running.is_empty() && self.unfocused_since.is_some_and(|t| now - t > 600.0);
        let input = PresenceInput {
            project: self.project.as_ref(),
            running,
            crashed,
            started_unix,
            away,
            secret: self.view.discord_secret,
            links: self.links.as_ref(),
        };
        self.presence.set(Some(discord::activity(&input)));
    }

    /// Установки плагинов: что поставилось или удалилось — к тому подключаются программа
    /// (сервер Java, темы) и список плагинов в настройках (для экспорта).
    fn poll_plugins(&mut self, ctx: &egui::Context, now: f64) {
        if self.plugins.poll() {
            ctx.request_repaint_after_secs(0.2);
        }
        for event in self.plugins.take_events() {
            let title = |id: &str| crate::plugins::entry(id).map(|e| e.name).unwrap_or_else(|| id.to_owned());
            match event {
                forma_plugins::Event::Installed(id) => {
                    let name = self.plugins.get(&id).map(|p| p.manifest.title().to_owned()).unwrap_or_else(|| title(&id));
                    self.toast(t!("toast-plugin-installed", name = name), false, now);
                }
                forma_plugins::Event::Failed { id, error } => {
                    if id == crate::plugins::JDTLS {
                        if let Some(l) = &mut self.lsp {
                            l.set_install_failed(error.clone());
                        }
                    }
                    self.toast(t!("toast-plugin-install-failed", name = title(&id), error = error), true, now);
                }
                forma_plugins::Event::Removed(_) => {}
            }
        }
        // Список установленных — в настройках, чтобы экспорт знал, какие плагины переносить.
        let installed: Vec<String> = self.plugins.installed().iter().map(|p| p.id().to_owned()).collect();
        if self.settings.plugins.installed != installed {
            self.settings.plugins.installed = installed;
        }
        if self.themes.set_plugin_dirs(crate::plugins::theme_dirs(&self.plugins, &self.settings.plugins)) {
            self.themes_rev += 1;
        }
        // Сервер Java — из включённого плагина jdtls; пока он ставится — показываем шаг.
        if let Some(l) = &mut self.lsp {
            l.set_server(crate::plugins::active_dir(&self.plugins, &self.settings.plugins, crate::plugins::JDTLS));
            l.set_installing(self.plugins.progress(crate::plugins::JDTLS));
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
                    self.refresh_configs();
                }
                SyncResult::Libraries(libs) => project.libraries = Some(libs),
                SyncResult::RunMods(run, logos) => {
                    project.rules = project.rules.with_mods(run.map());
                    self.console.set_rules(&project.rules);
                    let textures = logos.into_iter().map(|(id, img)| (id.clone(), self.ctx.load_texture(format!("mod-logo-{id}"), img, egui::TextureOptions::LINEAR))).collect();
                    crate::mods_view::set(Some(run.clone()), textures);
                    crate::mods_view::set_rules(&project.rules);
                    self.run_mods = Some(run);
                }
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
            recent: &self.settings.recent.projects,
            actions: &self.actions,
            keymap: &self.settings.keymap,
            shortcuts: self.modal.is_none(),
            next_tab_id: &mut self.layout.next_tab_id,
            cmds: &mut self.cmds,
            docs: &mut self.docs,
            console: &self.console,
            sync: &self.sync.state,
            run_mods: self.run_mods.as_deref(),
            lsp: self.lsp.as_mut(),
            configs: &self.configs,
            run_config: &mut self.view.run_config,
            states: &mut self.states,
            fonts: &self.settings.fonts,
            now,
        };
        let dock = &mut self.layout.dock;
        let style = &self.dock_style;
        egui::CentralPanel::default().frame(Frame::new().fill(self.pal.window_bg)).show(ui, |ui| {
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
        self.paint_backdrop(&ctx);
        self.modals(&ctx);
        self.toast_ui(&ctx);
        self.window_frame(&ctx);
        self.process_cmds(&ctx);
        self.update_title(&ctx);
        self.update_presence(now);
    }

    /// eframe вызывает это раз в полминуты и при выходе.
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        self.persist();
    }

    fn on_exit(&mut self) {
        self.persist();
        self.console.stop_all();
        // jdtls завершается по-хорошему, чтобы сохранить свою рабочую папку.
        self.lsp = None;
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.pal.window_bg.to_normalized_gamma_f32()
    }
}

/// Открывает папку в проводнике.
pub(super) fn open_folder(path: &Path) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer").arg(path).spawn();
    }
    #[cfg(not(windows))]
    {
        let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
        let _ = std::process::Command::new(opener).arg(path).spawn();
    }
}

pub(super) fn reveal_in_explorer(path: &Path) {
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

/// Задачи Gradle проекта для подсказок: `runClient`, `:fabric:build`… Задачи запуска — первыми.
fn gradle_tasks(project: &Project) -> Vec<String> {
    let Some(model) = &project.model else { return Vec::new() };
    let mut out: Vec<String> = model
        .projects
        .iter()
        .flat_map(|p| p.tasks.iter().chain(p.run_tasks.iter()).map(move |t| if p.path == ":" { t.clone() } else { format!("{}:{t}", p.path) }))
        .collect();
    out.sort_by_key(|t| (!t.rsplit(':').next().is_some_and(|n| n.starts_with("run")), t.clone()));
    out.dedup();
    out
}
