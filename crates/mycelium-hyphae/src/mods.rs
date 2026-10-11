//! Моды запуска: какие моды и библиотеки попадают в игру, откуда каждый взялся
//! (зависимость Gradle, папка `run/mods`, jar внутри другого jar) и чьи у них пакеты
//! и модули JPMS. Отсюда же — [`ModMap`], по которому стек-трейсы и лог узнают,
//! что класс `com.simibubi.create.Foo` — это мод Create, а не «какая-то библиотека».

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mycelium_project::model::Dependency;
use mycelium_project::{Loader, ModInfo, ModMap, Origin};

use crate::jar::{self, Jar};
use crate::meta::{self, Format, ModDependency};

/// Как глубоко разбираем jar внутри jar.
const MAX_NESTING: usize = 3;
/// Логотипы больше этого не читаем.
const MAX_LOGO: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ModKind {
    /// Свой мод (открытый проект).
    Own,
    Minecraft,
    /// Загрузчик и его части (NeoForge, Fabric Loader, Fabric API).
    Loader(Loader),
    /// Другой мод.
    Mod,
    /// Обычная библиотека.
    Library,
}

/// Откуда мод попал в запуск.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Found {
    /// Открытый проект.
    Project,
    /// Зависимость Gradle.
    Classpath,
    /// Папка `run/mods`.
    RunFolder,
    /// Внутри другого jar (jar-in-jar); `parent` — id мода или имя jar снаружи.
    Nested { parent: String },
}

/// Мод или библиотека запуска.
#[derive(Clone, Debug)]
pub struct RunMod {
    /// id мода; у библиотеки — имя артефакта.
    pub id: String,
    /// Название для людей.
    pub name: String,
    pub version: String,
    pub kind: ModKind,
    pub found: Found,
    /// jar на диске (у вложенного — внешний jar).
    pub jar: Option<PathBuf>,
    /// Путь внутри внешнего jar у вложенного.
    pub inner: Option<String>,
    /// Картинка логотипа (PNG как есть).
    pub logo: Option<Arc<[u8]>>,
    pub description: String,
    pub authors: String,
    pub format: Option<Format>,
    pub dependencies: Vec<ModDependency>,
    pub mixins: Vec<String>,
    /// Имя модуля JPMS (если jar его задаёт).
    pub module: Option<String>,
    /// Пакеты с классами через точку (у первого мода jar).
    pub packages: Vec<String>,
    /// Пространства имён ресурсов jar (`assets/<ns>`, `data/<ns>`) — у первого мода jar.
    pub namespaces: Vec<String>,
}

impl RunMod {
    fn new(id: impl Into<String>, name: impl Into<String>, kind: ModKind, found: Found) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            version: String::new(),
            kind,
            found,
            jar: None,
            inner: None,
            logo: None,
            description: String::new(),
            authors: String::new(),
            format: None,
            dependencies: Vec::new(),
            mixins: Vec::new(),
            module: None,
            packages: Vec::new(),
            namespaces: Vec::new(),
        }
    }

    /// Происхождение кода и ресурсов этого мода.
    pub fn origin(&self) -> Origin {
        match self.kind {
            ModKind::Own => Origin::OwnMod,
            ModKind::Minecraft => Origin::Minecraft,
            ModKind::Loader(l) => mycelium_project::origin::loader_origin(l),
            ModKind::Mod => Origin::OtherMod(Some(self.id.clone())),
            ModKind::Library => Origin::Library,
        }
    }
}

/// Откуда брать моды запуска.
pub struct RunInput<'a> {
    pub root: &'a Path,
    pub info: &'a ModInfo,
    /// Classpath запуска (`runtimeClasspath`, у старой модели — classpath компиляции).
    pub classpath: &'a [Dependency],
}

/// Все моды запуска.
#[derive(Clone, Debug, Default)]
pub struct RunMods {
    pub mods: Vec<RunMod>,
    by_id: HashMap<String, usize>,
    /// Мод встретился ещё раз: id и где (jar или «jar!/путь»). Игра на таком упадёт.
    pub duplicates: Vec<(String, String)>,
    map: Arc<ModMap>,
}

impl RunMods {
    /// Разбирает моды запуска: свой мод, classpath, `run/mods` и всё вложенное.
    pub fn build(input: &RunInput) -> Self {
        let mut out = RunMods::default();
        if let Some(own) = own_mod(input.root, input.info) {
            out.add(own);
        }
        for dep in input.classpath.iter().filter(|d| d.project.is_none()) {
            out.scan_file(&dep.file, Found::Classpath, Some(dep), input.info);
        }
        for dir in run_folders(input.root, input.info) {
            let mut jars: Vec<PathBuf> = std::fs::read_dir(&dir)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("jar")))
                .collect();
            jars.sort();
            for jar in jars {
                out.scan_file(&jar, Found::RunFolder, None, input.info);
            }
        }
        out.map = Arc::new(out.make_map());
        out
    }

    pub fn get(&self, id: &str) -> Option<&RunMod> {
        self.by_id.get(id).map(|&i| &self.mods[i])
    }

    /// Пакеты и модули модов — для правил происхождения.
    pub fn map(&self) -> Arc<ModMap> {
        self.map.clone()
    }

    /// Только моды (без Minecraft, загрузчика и библиотек), свой — первым.
    pub fn mods(&self) -> impl Iterator<Item = &RunMod> {
        self.mods.iter().filter(|m| matches!(m.kind, ModKind::Own | ModKind::Mod))
    }

    fn add(&mut self, m: RunMod) -> bool {
        // Библиотеки и вложенные jar встречаются много раз — из вложенных загрузчик сам
        // выбирает одну версию. Ошибка — только мод, дважды лежащий на верхнем уровне.
        if let Some(&i) = self.by_id.get(&m.id) {
            let top = |f: &Found| !matches!(f, Found::Nested { .. });
            if m.kind == ModKind::Mod && self.mods[i].kind == ModKind::Mod && top(&m.found) && top(&self.mods[i].found) {
                let place = match (&m.jar, &m.inner) {
                    (Some(j), Some(inner)) => format!("{}!/{inner}", j.display()),
                    (Some(j), None) => j.display().to_string(),
                    _ => String::new(),
                };
                self.duplicates.push((m.id.clone(), place));
            }
            return false;
        }
        self.by_id.insert(m.id.clone(), self.mods.len());
        self.mods.push(m);
        true
    }

    fn scan_file(&mut self, path: &Path, found: Found, dep: Option<&Dependency>, info: &ModInfo) {
        match Jar::open(path) {
            Ok(mut jar) => self.scan(&mut jar, path, None, found, dep, info, 0),
            Err(e) => log::debug!("Hyphae: cannot read {}: {e}", path.display()),
        }
    }

    /// Разбирает jar: Minecraft, моды из метаданных или библиотека; потом вложенные jar.
    #[allow(clippy::too_many_arguments)]
    fn scan(&mut self, jar: &mut Jar, file: &Path, inner: Option<&str>, found: Found, dep: Option<&Dependency>, info: &ModInfo, depth: usize) {
        let file_name = inner.and_then(|i| i.rsplit('/').next()).map(str::to_owned).unwrap_or_else(|| file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
        let meta = meta::read(jar);
        let place = |m: &mut RunMod| {
            m.jar = Some(file.to_path_buf());
            m.inner = inner.map(str::to_owned);
        };
        let mut parent = file_name.clone();
        if jar.has("net/minecraft/client/Minecraft.class") || jar.has("net/minecraft/server/MinecraftServer.class") {
            let mut mc = RunMod::new("minecraft", "Minecraft", ModKind::Minecraft, found.clone());
            mc.version = info.minecraft.clone().unwrap_or_default();
            place(&mut mc);
            self.add(mc);
            // ModDevGradle кладёт NeoForge в один jar с Minecraft.
            if jar.has_prefix("net/neoforged/neoforge/") {
                let mut neo = RunMod::new("neoforge", "NeoForge", ModKind::Loader(Loader::NeoForge), found.clone());
                neo.version = info.loader_version.clone().unwrap_or_default();
                place(&mut neo);
                self.add(neo);
            }
            parent = "minecraft".into();
        } else if let Some(meta) = meta.as_ref().filter(|m| !m.mods.is_empty()) {
            let module = jar.module_name();
            let packages = jar.packages();
            let namespaces = resource_namespaces(jar);
            for (i, mm) in meta.mods.iter().enumerate() {
                let kind = kind_of(&mm.id, meta.format);
                let mut m = RunMod::new(mm.id.clone(), mm.name.clone(), kind, found.clone());
                m.version = mm.version.clone();
                m.description = mm.description.clone();
                m.authors = mm.authors.clone();
                m.format = Some(meta.format);
                m.dependencies = mm.dependencies.clone();
                m.logo = find_logo(jar, &mm.id, mm.logo.as_deref());
                place(&mut m);
                if i == 0 {
                    m.mixins = meta.mixins.clone();
                    m.module = module.clone();
                    m.packages = packages.clone();
                    m.namespaces = namespaces.clone();
                }
                self.add(m);
            }
            parent = meta.mods[0].id.clone();
        } else {
            // Библиотека: имя и версия — из координат Gradle, манифеста или имени файла.
            let manifest = jar.manifest();
            let auto = jar::automatic_module_name(&file_name);
            let id = dep.and_then(|d| d.name.clone()).unwrap_or_else(|| if auto.is_empty() { file_name.clone() } else { auto.clone() });
            let kind = dep.and_then(|d| d.group.as_deref()).and_then(loader_group).map(ModKind::Loader).unwrap_or(ModKind::Library);
            let mut lib = RunMod::new(id.clone(), id, kind, found.clone());
            lib.version = dep.and_then(|d| d.version.clone()).or_else(|| manifest.get("Implementation-Version").cloned()).unwrap_or_default();
            lib.module = jar.module_name().or(Some(auto).filter(|a| !a.is_empty()));
            place(&mut lib);
            self.add(lib);
        }
        // Вложенные jar (jar-in-jar).
        let nested = meta.map(|m| m.nested).unwrap_or_default();
        if depth >= MAX_NESTING {
            return;
        }
        for path in nested {
            let Ok(bytes) = jar.read(&path) else { continue };
            let Ok(mut inner_jar) = Jar::from_bytes(bytes) else { continue };
            let inner_path = match inner {
                Some(outer) => format!("{outer}!/{path}"),
                None => path.clone(),
            };
            self.scan(&mut inner_jar, file, Some(&inner_path), Found::Nested { parent: parent.clone() }, None, info, depth + 1);
        }
    }

    fn make_map(&self) -> ModMap {
        let mut map = ModMap::default();
        // Сокращение пакета → полный пакет; одно сокращение у разных пакетов — не годится.
        let mut abbreviated: HashMap<String, Option<String>> = HashMap::new();
        // Чужое пространство имён → какие моды его кладут.
        let mut shippers: HashMap<String, Vec<Origin>> = HashMap::new();
        for m in &self.mods {
            let origin = m.origin();
            for p in &m.packages {
                let full = abbreviated.entry(mycelium_project::abbreviate_package(p)).or_insert_with(|| Some(p.clone()));
                if full.as_deref() != Some(p.as_str()) {
                    *full = None;
                }
            }
            for c in &m.mixins {
                map.mixins.entry(c.clone()).or_insert_with(|| origin.clone());
            }
            if matches!(m.kind, ModKind::Own | ModKind::Mod) {
                for ns in m.namespaces.iter().filter(|ns| self.get(ns).is_none()) {
                    let list = shippers.entry(ns.clone()).or_default();
                    if !list.contains(&origin) {
                        list.push(origin.clone());
                    }
                }
            }
            if matches!(m.kind, ModKind::Minecraft | ModKind::Library | ModKind::Own) {
                // Minecraft и свой мод узнаются по своим правилам, библиотеки — по умолчанию.
                continue;
            }
            for p in &m.packages {
                map.packages.entry(p.clone()).or_insert_with(|| origin.clone());
            }
            map.modules.entry(m.id.clone()).or_insert_with(|| origin.clone());
            if let Some(module) = &m.module {
                map.modules.entry(module.clone()).or_insert_with(|| origin.clone());
            }
        }
        map.abbreviated = abbreviated.into_iter().filter_map(|(a, full)| Some((a, full?))).collect();
        map.namespaces = shippers.into_iter().filter(|(_, list)| list.len() == 1).map(|(ns, mut list)| (ns, list.remove(0))).collect();
        map
    }
}

/// Пространства имён ресурсов jar: `assets/<ns>/…` и `data/<ns>/…`.
fn resource_namespaces(jar: &Jar) -> Vec<String> {
    let mut out: Vec<String> = jar
        .entries()
        .iter()
        .filter_map(|e| e.strip_prefix("assets/").or_else(|| e.strip_prefix("data/")))
        .filter_map(|rest| rest.split_once('/').map(|(ns, _)| ns.to_owned()))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Вид мода по id: Minecraft, части загрузчика или обычный мод.
fn kind_of(id: &str, format: Format) -> ModKind {
    match id {
        "minecraft" => ModKind::Minecraft,
        "neoforge" | "fml" => ModKind::Loader(Loader::NeoForge),
        "forge" => ModKind::Loader(Loader::Forge),
        "fabricloader" | "fabric-api" | "fabric" | "java" => ModKind::Loader(Loader::Fabric),
        "quilt_loader" => ModKind::Loader(Loader::Quilt),
        // Модули Fabric API: `fabric-api-base`, `fabric-networking-api-v1`…
        id if format == Format::Fabric && id.starts_with("fabric-") => ModKind::Loader(Loader::Fabric),
        _ => ModKind::Mod,
    }
}

/// Группа Maven частей загрузчика.
fn loader_group(group: &str) -> Option<Loader> {
    if group.starts_with("net.neoforged") || group.starts_with("cpw.mods") {
        Some(Loader::NeoForge)
    } else if group.starts_with("net.minecraftforge") {
        Some(Loader::Forge)
    } else if group == "net.fabricmc" || group.starts_with("net.fabricmc.fabric-api") {
        Some(Loader::Fabric)
    } else if group.starts_with("org.quiltmc") {
        Some(Loader::Quilt)
    } else {
        None
    }
}

/// Логотип мода: файл из метаданных, а если его нет (шаблонное `examplemod.png`) —
/// привычные имена в корне jar и в ассетах мода.
fn find_logo(jar: &mut Jar, id: &str, declared: Option<&str>) -> Option<Arc<[u8]>> {
    let candidates = [
        declared.map(|p| p.trim_start_matches('/').to_owned()),
        Some("icon.png".to_owned()),
        Some("logo.png".to_owned()),
        Some(format!("{id}-icon.png")),
        Some(format!("{id}_icon.png")),
        Some(format!("{id}.png")),
        Some(format!("assets/{id}/icon.png")),
        Some("pack.png".to_owned()),
    ];
    let path = candidates.into_iter().flatten().find(|p| jar.has(p))?;
    let bytes = jar.read(&path).ok()?;
    (bytes.len() <= MAX_LOGO).then(|| Arc::from(bytes))
}

/// Свой мод — из распознанного проекта.
fn own_mod(root: &Path, info: &ModInfo) -> Option<RunMod> {
    let id = info.mod_id.clone()?;
    let mut m = RunMod::new(id.clone(), info.mod_name.clone().unwrap_or(id), ModKind::Own, Found::Project);
    m.version = info.mod_version.clone().unwrap_or_default();
    m.logo = info.icon.as_ref().and_then(|p| std::fs::read(root.join(p)).ok()).filter(|b| b.len() <= MAX_LOGO).map(Arc::from);
    m.packages = info.packages.clone();
    Some(m)
}

/// Папки `run/mods` корня и подпроектов (у мультилоадерных проектов они у каждого загрузчика свои).
fn run_folders(root: &Path, info: &ModInfo) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::iter::once(root.to_path_buf()).chain(info.modules.iter().map(|m| root.join(&m.path))).map(|d| d.join("run").join("mods")).filter(|d| d.is_dir()).collect();
    dirs.sort();
    dirs.dedup();
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// jar в памяти из пар «путь → содержимое».
    fn zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            for (name, data) in files {
                w.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
                w.write_all(data).unwrap();
            }
            w.finish().unwrap();
        }
        buf.into_inner()
    }

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hyphae-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    const CREATE: &str = "[[mods]]\nmodId=\"create\"\nversion=\"6.0.10\"\ndisplayName=\"Create\"\nlogoFile=\"icon.png\"\n[[mixins]]\nconfig=\"create.mixins.json\"\n";

    #[test]
    fn mods_libraries_nested_and_duplicates() {
        let dir = temp("run");
        let lib = zip(&[("META-INF/MANIFEST.MF", b"Manifest-Version: 1.0\nAutomatic-Module-Name: com.tterrag.registrate\nImplementation-Version: 1.3\n"), ("com/tterrag/registrate/Registrate.class", b"")]);
        let ponder = zip(&[("META-INF/neoforge.mods.toml", b"[[mods]]\nmodId=\"ponder\"\nversion=\"1.0\"\ndisplayName=\"Ponder\"\n"), ("net/createmod/ponder/Ponder.class", b"")]);
        let jarjar = br#"{"jars":[{"path":"META-INF/jarjar/registrate.jar"},{"path":"META-INF/jarjar/ponder.jar"}]}"#;
        let create = zip(&[
            ("META-INF/neoforge.mods.toml", CREATE.as_bytes()),
            ("META-INF/jarjar/metadata.json", jarjar),
            ("META-INF/jarjar/registrate.jar", &lib),
            ("META-INF/jarjar/ponder.jar", &ponder),
            ("icon.png", b"\x89PNG"),
            ("assets/create/lang/en_us.json", b"{}"),
            ("data/quark/recipe/x.json", b"{}"),
            ("com/simibubi/create/Create.class", b""),
            ("com/simibubi/create/content/Foo.class", b""),
            ("assets/minecraft/lang/en_us.json", b"{}"),
        ]);
        std::fs::write(dir.join("create.jar"), &create).unwrap();
        std::fs::create_dir_all(dir.join("run").join("mods")).unwrap();
        // Тот же мод ещё и в папке run/mods.
        std::fs::write(dir.join("run").join("mods").join("create-copy.jar"), &create).unwrap();
        let gson = zip(&[("com/google/gson/Gson.class", b"")]);
        std::fs::write(dir.join("gson-2.10.1.jar"), &gson).unwrap();

        let info = ModInfo { mod_id: Some("testmod".into()), mod_name: Some("Test Mod".into()), packages: vec!["com.holo.testmod".into()], ..ModInfo::default() };
        let classpath = vec![
            Dependency { file: dir.join("create.jar"), group: Some("com.simibubi.create".into()), name: Some("create-1.21.1".into()), version: Some("6.0.10".into()), project: None },
            Dependency { file: dir.join("gson-2.10.1.jar"), group: Some("com.google.code.gson".into()), name: Some("gson".into()), version: Some("2.10.1".into()), project: None },
        ];
        let run = RunMods::build(&RunInput { root: &dir, info: &info, classpath: &classpath });

        let ids: Vec<&str> = run.mods.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, vec!["testmod", "create", "registrate", "ponder", "gson"]);
        let create = run.get("create").unwrap();
        assert_eq!((create.name.as_str(), create.version.as_str()), ("Create", "6.0.10"));
        assert_eq!(create.logo.as_deref(), Some(&b"\x89PNG"[..]));
        assert_eq!(create.mixins, vec!["create.mixins.json"]);
        assert_eq!(create.packages, vec!["com.simibubi.create", "com.simibubi.create.content"]);
        let ponder = run.get("ponder").unwrap();
        assert_eq!(ponder.found, Found::Nested { parent: "create".into() });
        assert_eq!(ponder.inner.as_deref(), Some("META-INF/jarjar/ponder.jar"));
        let registrate = run.get("registrate").unwrap();
        assert_eq!((registrate.kind, registrate.module.as_deref(), registrate.version.as_str()), (ModKind::Library, Some("com.tterrag.registrate"), "1.3"));
        assert_eq!(run.get("gson").unwrap().module.as_deref(), Some("gson"), "имя модуля из имени файла");
        assert_eq!(run.mods().map(|m| m.id.as_str()).collect::<Vec<_>>(), vec!["testmod", "create", "ponder"]);
        assert_eq!(run.duplicates.len(), 1, "второй Create из run/mods");
        assert_eq!(run.duplicates[0].0, "create");

        let map = run.map();
        assert_eq!(map.packages.get("com.simibubi.create.content"), Some(&Origin::OtherMod(Some("create".into()))));
        assert_eq!(map.packages.get("net.createmod.ponder"), Some(&Origin::OtherMod(Some("ponder".into()))));
        assert!(!map.packages.contains_key("com.google.gson"), "библиотеки узнаются по умолчанию");
        assert_eq!(map.modules.get("create"), Some(&Origin::OtherMod(Some("create".into()))));
        assert_eq!(map.namespaces.get("quark"), Some(&Origin::OtherMod(Some("create".into()))), "чужое пространство только у Create");
        assert!(!map.namespaces.contains_key("create"), "пространство с id мода — и так его");
        assert!(!map.namespaces.contains_key("minecraft") || run.get("minecraft").is_none());
        assert_eq!(map.mixins.get("create.mixins.json"), Some(&Origin::OtherMod(Some("create".into()))));
        assert_eq!(map.abbreviated.get("co.si.cr.co").map(String::as_str), Some("com.simibubi.create.content"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn moddev_jar_holds_minecraft_and_neoforge() {
        let dir = temp("mdg");
        let mc = zip(&[("net/minecraft/client/Minecraft.class", b""), ("net/neoforged/neoforge/common/NeoForge.class", b"")]);
        std::fs::write(dir.join("neoforge-21.1.252.jar"), &mc).unwrap();
        let info = ModInfo { minecraft: Some("1.21.1".into()), loader_version: Some("21.1.252".into()), ..ModInfo::default() };
        let classpath = vec![Dependency { file: dir.join("neoforge-21.1.252.jar"), ..Dependency::default() }];
        let run = RunMods::build(&RunInput { root: &dir, info: &info, classpath: &classpath });
        let kinds: Vec<(&str, ModKind, &str)> = run.mods.iter().map(|m| (m.id.as_str(), m.kind, m.version.as_str())).collect();
        assert_eq!(kinds, vec![("minecraft", ModKind::Minecraft, "1.21.1"), ("neoforge", ModKind::Loader(Loader::NeoForge), "21.1.252")]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
