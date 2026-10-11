//! Открытый проект мода. Пока это просто папка; на втором этапе здесь появятся
//! загрузчик (Fabric/NeoForge), версия Minecraft и модель Gradle-проекта.

use std::path::PathBuf;

pub struct Project {
    pub root: PathBuf,
    pub name: String,
}

impl Project {
    pub fn open(root: PathBuf) -> Result<Self, String> {
        if !root.is_dir() {
            return Err(root.display().to_string());
        }
        let name = root.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| root.display().to_string());
        Ok(Self { root, name })
    }
}
