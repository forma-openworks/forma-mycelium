//! Проект мода в Forma Mycelium: точка инициализации `.mycelium`, распознавание мода,
//! модель Gradle, внешние библиотеки и правила происхождения кода.

pub mod detect;
pub mod libraries;
pub mod model;
pub mod origin;
pub mod store;

use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use detect::{Language, Loader, ModInfo};
pub use libraries::Libraries;
pub use model::GradleModel;
pub use origin::{ModMap, Origin, OriginRules, abbreviate_package};
pub use store::ProjectMeta;

/// Открытый проект.
pub struct Project {
    pub root: PathBuf,
    pub meta: ProjectMeta,
    pub info: ModInfo,
    pub rules: OriginRules,
    /// Точку инициализации только что создали (проект открыт впервые).
    pub created: bool,
    /// Модель от Gradle (если её уже выгружали).
    pub model: Option<GradleModel>,
    /// Внешние библиотеки, разложенные по происхождению (строятся в фоне по модели).
    pub libraries: Option<Arc<Libraries>>,
}

impl Project {
    /// Открывает папку как проект: создаёт `.mycelium`, если её нет, и распознаёт мод.
    pub fn open(root: &Path) -> std::io::Result<Self> {
        let root = std::path::absolute(root)?;
        let (meta, created) = store::open_or_init(&root)?;
        let info = detect::detect(&root);
        let rules = OriginRules::from_info(&info);
        let model = GradleModel::load(&root);
        Ok(Self { root, meta, info, rules, created, model, libraries: None })
    }

    /// Перечитывает файлы проекта (например, после правки build.gradle).
    pub fn redetect(&mut self) {
        self.info = detect::detect(&self.root);
        self.rules = OriginRules::from_info(&self.info);
    }

    pub fn name(&self) -> &str {
        &self.meta.name
    }

    /// Путь внутри проекта → относительный (с прямыми слэшами — одинаково на всех системах).
    pub fn relative(&self, path: &Path) -> Option<PathBuf> {
        path.strip_prefix(&self.root).ok().map(Path::to_path_buf)
    }
}
