//! Рабочие пространства — вкладки над интерфейсом («Код», «Карта», «Ассеты», «Отладка»),
//! как Layout / Modeling / Shading в Blender. У каждого своя раскладка областей.

use std::path::Path;

use forma_i18n::t;
use forma_layout::{AreaId, DockState, Node, SplitDir, Surface};
use serde::{Deserialize, Serialize};

use crate::editors::{EditorKind, EditorTab};

/// Встроенные пространства: их раскладку можно сбросить к исходной.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Builtin {
    Code,
    Map,
    Assets,
    Debug,
}

impl Builtin {
    pub const ALL: [Builtin; 4] = [Builtin::Code, Builtin::Map, Builtin::Assets, Builtin::Debug];

    pub fn title(self) -> String {
        match self {
            Builtin::Code => t!("workspace-code"),
            Builtin::Map => t!("workspace-map"),
            Builtin::Assets => t!("workspace-assets"),
            Builtin::Debug => t!("workspace-debug"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Workspace {
    /// Из какого шаблона создано (для сброса раскладки).
    pub builtin: Option<Builtin>,
    /// Своё имя; без него показывается имя шаблона на текущем языке.
    pub name: Option<String>,
    pub surface: Surface<EditorTab>,
}

impl Workspace {
    pub fn title(&self) -> String {
        match (&self.name, self.builtin) {
            (Some(name), _) => name.clone(),
            (None, Some(b)) => b.title(),
            (None, None) => t!("workspace-untitled"),
        }
    }
}

/// Выдаёт номера областей и вкладок при сборке раскладок.
pub struct Ids<'a> {
    pub dock: &'a mut DockState<EditorTab>,
    pub next_tab: &'a mut u64,
}

impl Ids<'_> {
    fn area(&mut self, kinds: &[EditorKind]) -> Node<EditorTab> {
        let id: AreaId = self.dock.new_area_id();
        Node::area(id, kinds.iter().map(|k| EditorTab::new(*k, self.next_tab)).collect())
    }
}

/// Исходная раскладка встроенного пространства.
pub fn builtin_surface(b: Builtin, ids: &mut Ids) -> Surface<EditorTab> {
    use EditorKind::*;
    let h = SplitDir::Horizontal;
    let v = SplitDir::Vertical;
    let root = match b {
        // Проект слева, код в центре, консоль и исключения снизу.
        Builtin::Code => {
            let left = ids.area(&[Project]);
            let code = ids.area(&[Code]);
            let bottom = ids.area(&[Console, Exceptions]);
            Node::split(h, 0.2, left, Node::split(v, 0.72, code, bottom))
        }
        // Карта мода на весь экран, проект узкой колонкой.
        Builtin::Map => {
            let left = ids.area(&[Project]);
            let map = ids.area(&[ModMap]);
            Node::split(h, 0.18, left, map)
        }
        // Проект, превью модели и код рядом.
        Builtin::Assets => {
            let left = ids.area(&[Project]);
            let preview = ids.area(&[ModelPreview]);
            let code = ids.area(&[Code]);
            Node::split(h, 0.2, left, Node::split(h, 0.58, preview, code))
        }
        // Код и исключения с модами запуска сверху, консоль снизу.
        Builtin::Debug => {
            let code = ids.area(&[Code]);
            let exceptions = ids.area(&[Exceptions, RunMods]);
            let console = ids.area(&[Console]);
            Node::split(v, 0.64, Node::split(h, 0.6, code, exceptions), console)
        }
    };
    Surface::new(root)
}

/// Набор пространств при первом запуске.
pub fn default_workspaces(ids: &mut Ids) -> Vec<Workspace> {
    Builtin::ALL.iter().map(|&b| Workspace { builtin: Some(b), name: None, surface: builtin_surface(b, ids) }).collect()
}

/// Раскладка интерфейса: рабочие пространства, отдельные окна и счётчики номеров.
#[derive(Serialize, Deserialize)]
pub struct Layout {
    pub workspaces: Vec<Workspace>,
    pub active: usize,
    pub dock: DockState<EditorTab>,
    pub next_tab_id: u64,
}

impl Layout {
    /// Заводская раскладка: четыре встроенных пространства.
    pub fn factory() -> Self {
        let mut dock = DockState::default();
        let mut next_tab = 0;
        let workspaces = default_workspaces(&mut Ids { dock: &mut dock, next_tab: &mut next_tab });
        Self { workspaces, active: 0, dock, next_tab_id: next_tab }
    }

    pub fn ids(&mut self) -> Ids<'_> {
        Ids { dock: &mut self.dock, next_tab: &mut self.next_tab_id }
    }

    pub fn surfaces(&self) -> impl Iterator<Item = &Surface<EditorTab>> {
        self.workspaces.iter().map(|w| &w.surface).chain(self.dock.windows.iter().map(|w| &w.surface))
    }

    /// Чинит загруженную раскладку и выставляет счётчики выше уже занятых номеров.
    pub fn sanitize(&mut self) {
        if self.workspaces.is_empty() {
            self.workspaces = default_workspaces(&mut self.ids());
        }
        for w in &mut self.workspaces {
            w.surface.sanitize();
        }
        self.dock.sanitize();
        let max_area = self.workspaces.iter().map(|w| w.surface.max_area_id()).max().unwrap_or(0);
        self.dock.reserve_ids(max_area);
        let max_tab = self.surfaces().flat_map(|s| s.areas().into_iter().flat_map(|a| a.tabs.iter().map(|t| t.id))).max().unwrap_or(0);
        self.next_tab_id = self.next_tab_id.max(max_tab);
        self.active = self.active.min(self.workspaces.len() - 1);
    }

    /// Копия без открытых файлов — для стандартной раскладки, которая подходит любому проекту.
    /// Вкладки файлов превращаются в пустые вкладки «Код», по одной на область.
    pub fn without_files(&self) -> Option<Layout> {
        let value = serde_json::to_value(self).ok()?;
        let mut copy: Layout = serde_json::from_value(value).ok()?;
        let strip = |s: &mut Surface<EditorTab>| {
            s.for_each_area_mut(|a| {
                let mut seen_code = false;
                a.tabs.retain_mut(|t| {
                    if t.kind != EditorKind::Code {
                        return true;
                    }
                    t.file = None;
                    t.entry = None;
                    !std::mem::replace(&mut seen_code, true)
                });
                a.clamp_active();
            });
        };
        for w in &mut copy.workspaces {
            strip(&mut w.surface);
        }
        for w in &mut copy.dock.windows {
            strip(&mut w.surface);
        }
        copy.sanitize();
        Some(copy)
    }
}

/// Что ещё проект помнит об интерфейсе, кроме раскладки.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewState {
    /// Раскрытые папки дерева проекта (пути относительно корня, через «/»).
    pub expanded: Vec<String>,
    /// Показывать скрытые файлы и папки.
    pub show_hidden: bool,
    /// Выбранная конфигурация запуска.
    pub run_config: Option<String>,
    /// Раздел «Внешние библиотеки» в дереве раскрыт.
    pub libs_open: bool,
    /// Секретный проект: в Discord не видно ни названия мода, ни его иконки и репозитория.
    pub discord_secret: bool,
}

/// Файл раскладки: `workspace.json` проекта, стандартная и глобальная раскладки.
#[derive(Deserialize)]
struct LayoutFile {
    version: u32,
    #[serde(flatten)]
    layout: Layout,
    #[serde(default)]
    view: ViewState,
}

#[derive(Serialize)]
struct LayoutFileRef<'a> {
    version: u32,
    #[serde(flatten)]
    layout: &'a Layout,
    view: &'a ViewState,
}

pub const LAYOUT_VERSION: u32 = 2;

/// Читает раскладку. Файл другой версии или испорченный — `None` (тогда берётся запасная).
pub fn read_layout(path: &Path) -> Option<(Layout, ViewState)> {
    let file: LayoutFile = crate::settings::read_json(path)?;
    if file.version != LAYOUT_VERSION {
        return None;
    }
    let mut layout = file.layout;
    layout.sanitize();
    Some((layout, file.view))
}

pub fn write_layout(path: &Path, layout: &Layout, view: &ViewState) {
    crate::settings::write_json(path, &LayoutFileRef { version: LAYOUT_VERSION, layout, view });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_workspaces_have_unique_ids() {
        let mut dock = DockState::default();
        let mut next_tab = 0;
        let spaces = default_workspaces(&mut Ids { dock: &mut dock, next_tab: &mut next_tab });
        assert_eq!(spaces.len(), 4);
        let mut areas: Vec<u64> = spaces.iter().flat_map(|w| w.surface.areas().into_iter().map(|a| a.id.0)).collect();
        let mut tabs: Vec<u64> = spaces.iter().flat_map(|w| w.surface.areas().into_iter().flat_map(|a| a.tabs.iter().map(|t| t.id))).collect();
        let (na, nt) = (areas.len(), tabs.len());
        areas.sort();
        areas.dedup();
        tabs.sort();
        tabs.dedup();
        assert_eq!(areas.len(), na, "номера областей не повторяются");
        assert_eq!(tabs.len(), nt, "номера вкладок не повторяются");
        assert!(spaces.iter().all(|w| !w.surface.is_empty()));
    }

    #[test]
    fn default_layout_drops_open_files() {
        let mut layout = Layout::factory();
        let code_area = layout.workspaces[0].surface.areas()[1].id;
        let next = &mut layout.next_tab_id;
        let mut a = EditorTab::new(EditorKind::Code, next);
        a.file = Some("src/A.java".into());
        let mut b = EditorTab::new(EditorKind::Code, next);
        b.file = Some("src/B.java".into());
        let area = layout.workspaces[0].surface.area_mut(code_area).unwrap();
        area.tabs.push(a);
        area.tabs.push(b);
        let copy = layout.without_files().unwrap();
        let area = copy.workspaces[0].surface.area(code_area).unwrap();
        assert_eq!(area.tabs.len(), 1, "три вкладки «Код» схлопнулись в одну пустую");
        assert!(area.tabs[0].file.is_none());
    }

    #[test]
    fn layout_file_roundtrip_with_view() {
        let dir = std::env::temp_dir().join(format!("forma-layout-{}", std::process::id()));
        let path = dir.join("workspace.json");
        let layout = Layout::factory();
        let view = ViewState { expanded: vec!["src/main".into()], show_hidden: true, run_config: Some("client".into()), libs_open: true, discord_secret: false };
        write_layout(&path, &layout, &view);
        let (back, view_back) = read_layout(&path).unwrap();
        assert_eq!(back.workspaces.len(), 4);
        assert!(view_back.show_hidden);
        assert_eq!(view_back.expanded, vec!["src/main".to_owned()]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
