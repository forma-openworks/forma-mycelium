//! Установка jdtls (Eclipse JDT Language Server) и поиск JDK для его запуска.
//!
//! Архив скачивается системными `curl` и `tar`: в Windows 10+ они есть в System32,
//! так в программу не приходится тянуть HTTP- и TLS-библиотеки.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

/// Последняя сборка jdtls.
pub const JDTLS_URL: &str = "https://download.eclipse.org/jdtls/snapshots/jdt-language-server-latest.tar.gz";
/// Наименьшая версия Java, на которой запускается jdtls.
pub const MIN_JAVA: u32 = 21;

/// Папка jdtls: `%LOCALAPPDATA%\Forma\Mycelium\servers\jdtls`.
pub fn jdtls_dir() -> Option<PathBuf> {
    crate::settings::cache_dir().map(|d| d.join("servers").join("jdtls"))
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

#[derive(Clone, Debug, PartialEq)]
pub enum InstallState {
    Downloading { done: u64, total: Option<u64> },
    Extracting,
    Done,
    Failed(String),
}

/// Установка в фоне; состояние читается каждый кадр.
pub struct Installer {
    pub state: Arc<Mutex<InstallState>>,
}

impl Installer {
    pub fn start(dir: PathBuf) -> Self {
        let state = Arc::new(Mutex::new(InstallState::Downloading { done: 0, total: None }));
        let s = state.clone();
        std::thread::spawn(move || {
            let result = install(&dir, &s);
            *s.lock().expect("состояние установки") = match result {
                Ok(()) => InstallState::Done,
                Err(e) => InstallState::Failed(e),
            };
        });
        Self { state }
    }

    pub fn state(&self) -> InstallState {
        self.state.lock().expect("состояние установки").clone()
    }
}

/// Системная программа: в Windows — из System32 (у Git свои `tar` и `curl`, и GNU tar
/// принимает `C:` в пути за имя сервера).
fn system_tool(name: &str) -> Command {
    #[cfg(windows)]
    {
        let root = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        let exe = root.join("System32").join(format!("{name}.exe"));
        let mut cmd = Command::new(if exe.is_file() { exe } else { PathBuf::from(name) });
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd
    }
    #[cfg(not(windows))]
    {
        Command::new(name)
    }
}

fn install(dir: &Path, state: &Mutex<InstallState>) -> Result<(), String> {
    let parent = dir.parent().ok_or("bad install dir")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let archive = parent.join("jdtls-download.tar.gz");
    let _ = std::fs::remove_file(&archive);

    // Размер — из заголовков, чтобы показывать проценты.
    let total = system_tool("curl")
        .args(["-sIL", JDTLS_URL])
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter_map(|l| l.split_once(':').filter(|(k, _)| k.eq_ignore_ascii_case("content-length")).and_then(|(_, v)| v.trim().parse::<u64>().ok()))
                .last()
        });

    let mut child = system_tool("curl")
        .args(["-L", "-f", "-s", "-S", "-o"])
        .arg(&archive)
        .arg(JDTLS_URL)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("curl: {e}"))?;
    let status = loop {
        let done = std::fs::metadata(&archive).map(|m| m.len()).unwrap_or(0);
        *state.lock().expect("состояние установки") = InstallState::Downloading { done, total };
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(150)),
            Err(e) => return Err(e.to_string()),
        }
    };
    if !status.success() {
        let mut err = String::new();
        if let Some(mut s) = child.stderr.take() {
            use std::io::Read;
            let _ = s.read_to_string(&mut err);
        }
        let _ = std::fs::remove_file(&archive);
        return Err(format!("curl: {}", err.trim()));
    }

    *state.lock().expect("состояние установки") = InstallState::Extracting;
    // Распаковываем рядом и только потом подменяем папку: прерванная установка не ломает прежнюю.
    let staging = parent.join("jdtls-new");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    let out = system_tool("tar").arg("-xzf").arg(&archive).arg("-C").arg(&staging).output().map_err(|e| format!("tar: {e}"))?;
    let _ = std::fs::remove_file(&archive);
    if !out.status.success() {
        return Err(format!("tar: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    if launcher(&staging).is_none() {
        return Err("archive has no plugins/org.eclipse.equinox.launcher_*.jar".into());
    }
    let _ = std::fs::remove_dir_all(dir);
    std::fs::rename(&staging, dir).map_err(|e| e.to_string())?;
    Ok(())
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
