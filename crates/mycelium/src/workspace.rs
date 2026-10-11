//! Рабочие пространства — вкладки над интерфейсом («Код», «Карта», «Ассеты», «Отладка»),
//! как Layout / Modeling / Shading в Blender. У каждого своя раскладка областей.

use mycelium_i18n::t;
use mycelium_layout::{AreaId, DockState, Node, SplitDir, Surface};
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
        // Код и исключения сверху, консоль снизу.
        Builtin::Debug => {
            let code = ids.area(&[Code]);
            let exceptions = ids.area(&[Exceptions]);
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

/// Файл раскладки: пространства, отдельные окна, счётчики номеров.
#[derive(Serialize, Deserialize)]
pub struct LayoutFile {
    pub version: u32,
    pub workspaces: Vec<Workspace>,
    pub active: usize,
    pub dock: DockState<EditorTab>,
    pub next_tab_id: u64,
}

pub const LAYOUT_VERSION: u32 = 1;

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
}
