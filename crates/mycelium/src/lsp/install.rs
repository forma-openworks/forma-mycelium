//! jdtls (Eclipse JDT Language Server) — плагин Forma Mycelium (`plugins.rs`): проверка
//! его папки, нужная ему Java и поиск JDK для запуска.

use std::path::{Path, PathBuf};

/// Последняя сборка jdtls.
pub const JDTLS_URL: &str = "https://download.eclipse.org/jdtls/snapshots/jdt-language-server-latest.tar.gz";
/// Наименьшая версия Java, на которой запускается jdtls.
pub const MIN_JAVA: u32 = 21;

/// Годится ли папка как jdtls: в ней есть jar запуска.
pub fn validate(dir: &Path) -> Result<(), String> {
    launcher(dir).map(|_| ()).ok_or_else(|| "the archive has no plugins/org.eclipse.equinox.launcher_*.jar".to_owned())
}

/// Jar запуска jdtls (`plugins/org.eclipse.equinox.launcher_*.jar`), если jdtls установлен.
pub fn launcher(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir.join("plugins"))
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.file_name().is_some_and(|n| {
            let n = n.to_string_lossy();
            n.starts_with("org.eclipse.equinox.launcher_") && n.ends_with(".jar")
        }))
}

/// Какая Java нужна этой сборке jdtls. Скрипт jdtls пишет «21», но модули Eclipse
/// новых сборок требуют больше — смотрим `Bundle-RequiredExecutionEnvironment` во всех
/// плагинах. Ответ запоминается рядом с jdtls.
pub fn required_java(dir: &Path) -> u32 {
    let cache = dir.join(".mycelium-java");
    if let Some(v) = std::fs::read_to_string(&cache).ok().and_then(|s| s.trim().parse().ok()) {
        return v;
    }
    let mut need = MIN_JAVA;
    for e in std::fs::read_dir(dir.join("plugins")).into_iter().flatten().flatten() {
        let path = e.path();
        if path.extension().is_none_or(|x| x != "jar") {
            continue;
        }
        let Ok(manifest) = mycelium_project::libraries::read_entry(&path, "META-INF/MANIFEST.MF") else { continue };
        let text = String::from_utf8_lossy(&manifest);
        for line in text.lines().filter(|l| l.starts_with("Bundle-RequiredExecutionEnvironment:")) {
            for env in line.split([':', ',']).map(str::trim) {
                if let Some(v) = env.strip_prefix("JavaSE-").and_then(|v| v.parse::<u32>().ok()) {
                    need = need.max(v);
                }
            }
        }
    }
    let _ = std::fs::write(&cache, need.to_string());
    need
}

/// Папка настроек OSGi для этой системы.
pub fn config_dir(dir: &Path) -> PathBuf {
    let name = if cfg!(windows) {
        "config_win"
    } else if cfg!(target_os = "macos") {
        "config_mac"
    } else {
        "config_linux"
    };
    dir.join(name)
}

/// JDK: папка и основная версия (21, 25…).
#[derive(Clone, Debug, PartialEq)]
pub struct JavaHome {
    pub home: PathBuf,
    pub major: u32,
}

impl JavaHome {
    pub fn java(&self) -> PathBuf {
        self.home.join("bin").join(if cfg!(windows) { "java.exe" } else { "java" })
    }
}

/// Версия JDK из его файла `release`: `JAVA_VERSION="21.0.12.1"` → 21, `"1.8.0"` → 8.
pub fn jdk_version(home: &Path) -> Option<u32> {
    let text = std::fs::read_to_string(home.join("release")).ok()?;
    let line = text.lines().find(|l| l.starts_with("JAVA_VERSION="))?;
    let v = line.split_once('=')?.1.trim().trim_matches('"');
    let mut parts = v.split(['.', '_', '+', '-']);
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 { parts.next()?.parse().ok() } else { Some(first) }
}

/// Подходящий JDK версии не ниже `min`: сначала `preferred` (тулчейн проекта), потом
/// JAVA_HOME, потом JDK из кеша Gradle, IntelliJ и стандартных папок. Если `preferred`
/// не подходит — самая новая найденная версия.
pub fn find_jdk(preferred: Option<&Path>, min: u32) -> Option<JavaHome> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    candidates.extend(preferred.map(Path::to_path_buf));
    if let Some(h) = std::env::var_os("JAVA_HOME") {
        candidates.push(PathBuf::from(h));
    }
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join(".gradle").join("jdks"));
        roots.push(home.join(".jdks"));
    }
    for base in ["C:\\Program Files\\Java", "C:\\Program Files\\Eclipse Adoptium", "C:\\Program Files\\Microsoft", "C:\\Program Files\\Zulu", "/usr/lib/jvm", "/Library/Java/JavaVirtualMachines"] {
        roots.push(PathBuf::from(base));
    }
    for root in roots {
        for e in std::fs::read_dir(&root).into_iter().flatten().flatten() {
            let p = e.path();
            // macOS: Contents/Home внутри.
            let mac = p.join("Contents").join("Home");
            candidates.push(if mac.is_dir() { mac } else { p });
        }
    }
    let found: Vec<JavaHome> = candidates
        .into_iter()
        .filter_map(|home| {
            let major = jdk_version(&home)?;
            let java = home.join("bin").join(if cfg!(windows) { "java.exe" } else { "java" });
            (major >= min && java.is_file()).then_some(JavaHome { home, major })
        })
        .collect();
    // Тулчейн проекта подходит — берём его (он первый), иначе самую новую версию.
    if let Some(first) = found.first().filter(|j| preferred.is_some_and(|p| j.home == p)) {
        return Some(first.clone());
    }
    found.into_iter().max_by_key(|j| j.major)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_jdk_release_versions() {
        let dir = std::env::temp_dir().join(format!("mycelium-jdk-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (text, want) in [("JAVA_VERSION=\"21.0.12.1\"", 21), ("JAVA_VERSION=\"1.8.0_503\"", 8), ("IMPLEMENTOR=\"x\"\nJAVA_VERSION=\"25\"", 25)] {
            std::fs::write(dir.join("release"), text).unwrap();
            assert_eq!(jdk_version(&dir), Some(want), "{text}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
