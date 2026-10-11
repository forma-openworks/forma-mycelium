//! Модель проекта от Gradle: модули, исходники, зависимости с путями к jar, JDK тулчейна
//! и задачи запуска. Её выгружает init-скрипт Forma Mycelium в `.mycelium/cache/model.json`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Где лежит выгруженная модель.
pub const MODEL_FILE: &str = "cache/model.json";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GradleModel {
    /// Версия Gradle, которой выгружена модель.
    pub gradle: String,
    pub projects: Vec<GradleProject>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GradleProject {
    /// Путь Gradle: `:` у корня, `:fabric` у подпроекта.
    pub path: String,
    pub dir: PathBuf,
    pub source_sets: Vec<SourceSet>,
    pub dependencies: Vec<Dependency>,
    pub run_tasks: Vec<String>,
    /// Все задачи модуля — для подсказок при настройке запуска.
    pub tasks: Vec<String>,
    pub jdk: Option<Jdk>,
    /// Зависимости не разрешились (нет сети, ошибка в build.gradle).
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SourceSet {
    pub name: String,
    pub java: Vec<PathBuf>,
    pub kotlin: Vec<PathBuf>,
    pub resources: Vec<PathBuf>,
}

/// Зависимость на classpath компиляции.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Dependency {
    pub file: PathBuf,
    pub group: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    /// Другой подпроект этого же проекта (`:common`).
    pub project: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Jdk {
    pub home: PathBuf,
    pub version: String,
}

impl GradleModel {
    /// Читает выгруженную модель проекта. Нет файла или он испорчен — `None`.
    pub fn load(root: &Path) -> Option<Self> {
        crate::store::read_json(root, MODEL_FILE)
    }

    pub fn path(root: &Path) -> PathBuf {
        crate::store::dir(root).join(MODEL_FILE)
    }

    /// Все зависимости всех модулей без повторов (по пути к файлу), кроме своих подпроектов.
    pub fn dependencies(&self) -> Vec<&Dependency> {
        let mut seen = std::collections::HashSet::new();
        self.projects
            .iter()
            .flat_map(|p| p.dependencies.iter())
            .filter(|d| d.project.is_none() && seen.insert(d.file.clone()))
            .collect()
    }

    /// JDK тулчейна (первый найденный).
    pub fn jdk(&self) -> Option<&Jdk> {
        self.projects.iter().find_map(|p| p.jdk.as_ref())
    }

    /// Устарела ли модель: build-файлы менялись после выгрузки.
    pub fn is_stale(root: &Path) -> bool {
        let Some(model_time) = std::fs::metadata(Self::path(root)).and_then(|m| m.modified()).ok() else { return true };
        let mut files: Vec<PathBuf> = ["build.gradle", "build.gradle.kts", "settings.gradle", "settings.gradle.kts", "gradle.properties"]
            .iter()
            .map(|f| root.join(f))
            .collect();
        files.push(root.join("gradle/libs.versions.toml"));
        // build-файлы подпроектов.
        if let Ok(dirs) = std::fs::read_dir(root) {
            for d in dirs.flatten().filter(|d| d.path().is_dir()) {
                files.push(d.path().join("build.gradle"));
                files.push(d.path().join("build.gradle.kts"));
            }
        }
        files.iter().filter_map(|f| std::fs::metadata(f).and_then(|m| m.modified()).ok()).any(|t| t > model_time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_init_script_output() {
        let json = r#"{
            "gradle": "9.2.1",
            "projects": [{
                "path": ":", "dir": "C:\\mods\\testmod",
                "sourceSets": [{"name": "main", "java": ["C:\\mods\\testmod\\src\\main\\java"], "kotlin": [], "resources": []}],
                "dependencies": [
                    {"file": "C:\\mods\\testmod\\build\\moddev\\artifacts\\neoforge-21.1.252.jar"},
                    {"file": "C:\\g\\gson-2.10.1.jar", "group": "com.google.code.gson", "name": "gson", "version": "2.10.1"},
                    {"file": "C:\\g\\gson-2.10.1.jar", "group": "com.google.code.gson", "name": "gson", "version": "2.10.1"},
                    {"file": "C:\\mods\\testmod\\common\\build\\libs\\common.jar", "project": ":common"}
                ],
                "runTasks": ["runClient", "runData", "runServer"],
                "jdk": {"home": "C:\\jdks\\21", "version": "21"}
            }]
        }"#;
        let model: GradleModel = serde_json::from_str(json).unwrap();
        assert_eq!(model.projects[0].run_tasks, vec!["runClient", "runData", "runServer"]);
        assert_eq!(model.dependencies().len(), 2, "повтор и свой подпроект отброшены");
        assert_eq!(model.jdk().unwrap().version, "21");
    }
}
