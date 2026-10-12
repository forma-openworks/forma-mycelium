//! Внешние библиотеки проекта, разложенные по происхождению: Minecraft, загрузчик,
//! другие моды, обычные библиотеки и JDK. Содержимое jar показывается деревом пакетов,
//! а для классов находятся исходники — из `-sources.jar` рядом или в кеше Gradle.
//!
//! У ModDevGradle Minecraft и NeoForge собраны в один jar, поэтому группы строятся
//! не из jar целиком, а из «частей»: jar + какие пути из него показывать.

use std::collections::{BTreeSet, HashMap};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::detect::{Loader, ModInfo};
use crate::model::{Dependency, GradleModel, Jdk};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LibKind {
    /// Сам Minecraft (классы или ресурсы игры).
    Minecraft,
    /// Библиотеки Mojang, которые идут с игрой: brigadier, datafixerupper, authlib…
    Mojang,
    /// Загрузчик и его части: FML, шина событий, modlauncher, Fabric API.
    Loader(Loader),
    /// Другой мод.
    Mod { id: String },
    Library,
}

#[derive(Clone, Debug)]
pub struct Library {
    pub jar: PathBuf,
    pub sources: Option<PathBuf>,
    pub group: Option<String>,
    /// Имя для показа: артефакт или имя мода.
    pub name: String,
    pub version: Option<String>,
    pub kind: LibKind,
    /// В этом jar вместе с Minecraft лежит и загрузчик (ModDevGradle).
    pub with_loader: Option<Loader>,
    /// Файлы jar по алфавиту (без папок).
    pub entries: Arc<Vec<String>>,
}

/// Часть содержимого группы: jar и какие пути из него показывать (пусто — все).
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub lib: usize,
    pub prefixes: Vec<&'static str>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GroupKind {
    Minecraft,
    Loader(Loader),
    Mods,
    Libraries,
}

/// Группа во «Внешних библиотеках».
#[derive(Clone, Debug)]
pub struct Group {
    pub kind: GroupKind,
    pub version: Option<String>,
    /// Своё содержимое группы (пакеты и ресурсы).
    pub parts: Vec<Part>,
    /// Отдельные jar внутри группы.
    pub members: Vec<usize>,
}

/// Узел дерева содержимого.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    /// Имя для показа: `net.minecraft.world`, `assets/minecraft`, `Item`, `ruby.json`.
    pub name: String,
    /// Полный путь внутри jar: у папки — с `/` на конце.
    pub path: String,
    pub dir: bool,
    /// Из какого jar файл (у папок — первый jar, где она нашлась).
    pub lib: usize,
}

impl Node {
    pub fn is_class(&self) -> bool {
        !self.dir && self.path.ends_with(".class")
    }
}

const MINECRAFT_PREFIXES: &[&str] = &["net/minecraft/", "com/mojang/", "assets/minecraft/", "data/minecraft/"];
const NEOFORGE_PREFIXES: &[&str] = &["net/neoforged/", "assets/neoforge/", "data/neoforge/", "data/c/"];
const FORGE_PREFIXES: &[&str] = &["net/minecraftforge/", "assets/forge/", "data/forge/"];

pub struct Libraries {
    pub libs: Vec<Library>,
    pub groups: Vec<Group>,
    pub jdk: Option<Jdk>,
    /// Файлы jar с исходниками (читаются при первом обращении).
    source_entries: Mutex<HashMap<PathBuf, Arc<Vec<String>>>>,
}

impl Libraries {
    /// Разбирает зависимости модели: открывает каждый jar, узнаёт, чей он, и запоминает его файлы.
    pub fn build(model: &GradleModel, info: &ModInfo) -> Self {
        let mut deps: Vec<Dependency> = model.dependencies().into_iter().cloned().collect();
        // Ресурсы Minecraft у ModDevGradle лежат отдельным jar рядом и в classpath компиляции не входят.
        let extra: Vec<Dependency> = deps
            .iter()
            .filter_map(|d| d.file.parent())
            .filter(|dir| dir.ends_with("moddev/artifacts") || dir.ends_with("moddev\\artifacts"))
            .flat_map(|dir| std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()))
            .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().contains("client-extra")))
            .map(|file| Dependency { file, ..Dependency::default() })
            .collect();
        for d in extra {
            if !deps.iter().any(|x| x.file == d.file) {
                deps.push(d);
            }
        }

        let libs: Vec<Library> = deps.iter().filter_map(|d| inspect(d, info)).collect();
        let groups = make_groups(&libs, info);
        Self { libs, groups, jdk: model.jdk().cloned(), source_entries: Mutex::new(HashMap::new()) }
    }

    /// Дочерние узлы папки `dir` (`""` — корень) в наборе частей. Цепочки папок
    /// с единственной вложенной папкой сворачиваются в одну строку: `net.minecraft`.
    pub fn children(&self, parts: &[Part], dir: &str) -> Vec<Node> {
        let (dirs, files) = self.list(parts, dir);
        let mut out = Vec::with_capacity(dirs.len() + files.len());
        for (seg, lib) in dirs {
            let mut path = format!("{dir}{seg}/");
            let resource = is_resource_path(&path);
            let mut name = seg;
            // Сворачиваем цепочку, пока внутри ровно одна папка и нет файлов.
            loop {
                let (sub_dirs, sub_files) = self.list(parts, &path);
                if sub_files.is_empty() && sub_dirs.len() == 1 {
                    let (next, _) = sub_dirs.into_iter().next().expect("ровно одна");
                    name.push(if resource { '/' } else { '.' });
                    name.push_str(&next);
                    path.push_str(&next);
                    path.push('/');
                } else {
                    break;
                }
            }
            out.push(Node { name, path, dir: true, lib });
        }
        for (entry, lib) in files {
            let file = entry.rsplit('/').next().unwrap_or(&entry);
            let name = file.strip_suffix(".class").unwrap_or(file).to_owned();
            out.push(Node { name, path: entry, dir: false, lib });
        }
        out
    }

    /// Папки (имя, jar) и файлы (путь, jar) прямо внутри `dir`.
    fn list(&self, parts: &[Part], dir: &str) -> (Vec<(String, usize)>, Vec<(String, usize)>) {
        let mut dirs: BTreeSet<(String, usize)> = BTreeSet::new();
        let mut seen_dirs: BTreeSet<String> = BTreeSet::new();
        let mut files: Vec<(String, usize)> = Vec::new();
        for part in parts {
            let Some(lib) = self.libs.get(part.lib) else { continue };
            let entries = &lib.entries;
            let start = entries.partition_point(|e| e.as_str() < dir);
            for e in entries[start..].iter().take_while(|e| e.starts_with(dir)) {
                if !part.prefixes.is_empty() && !part.prefixes.iter().any(|p| e.starts_with(p)) {
                    continue;
                }
                let rest = &e[dir.len()..];
                match rest.split_once('/') {
                    Some((seg, _)) => {
                        if seen_dirs.insert(seg.to_owned()) {
                            dirs.insert((seg.to_owned(), part.lib));
                        }
                    }
                    None if is_shown_file(rest) => files.push((e.clone(), part.lib)),
                    None => {}
                }
            }
        }
        files.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
        files.dedup_by(|a, b| a.0 == b.0);
        let mut dirs: Vec<(String, usize)> = dirs.into_iter().collect();
        dirs.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
        (dirs, files)
    }

    /// Jar, в котором лежит класс (`net.minecraft.world.item.Item$Properties`), и путь к `.class`.
    pub fn find_class(&self, fqcn: &str) -> Option<(usize, String)> {
        let outer = fqcn.split('$').next().unwrap_or(fqcn);
        let entry = format!("{}.class", outer.replace('.', "/"));
        self.libs.iter().position(|l| l.entries.binary_search(&entry).is_ok()).map(|i| (i, entry))
    }

    /// Есть ли сам класс — вложенный (`KeyBindsList$KeyEntry`) тоже, а не только внешний.
    /// Имя — через точки или через `/`.
    pub fn has_class(&self, fqcn: &str) -> bool {
        let entry = format!("{}.class", fqcn.replace('.', "/"));
        self.libs.iter().any(|l| l.entries.binary_search(&entry).is_ok())
    }

    /// Классы с тем же коротким именем в других пакетах — так видно, куда класс переехал
    /// в этой версии игры (`…screens.controls.KeyBindsList` → `…screens.options.controls.KeyBindsList`).
    pub fn classes_named(&self, fqcn: &str) -> Vec<String> {
        let fqcn = fqcn.replace('/', ".");
        let simple = fqcn.rsplit('.').next().unwrap_or(&fqcn);
        let tail = format!("/{simple}.class");
        let mut out: Vec<String> = self
            .libs
            .iter()
            .flat_map(|l| l.entries.iter())
            .filter(|e| e.ends_with(&tail))
            .map(|e| e.trim_end_matches(".class").replace('/', "."))
            .filter(|c| *c != fqcn)
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// Исходник класса: jar с исходниками и путь внутри. `file` — имя файла из стек-трейса
    /// (`Item.java`); без него пробуем `.java` и `.kt` по имени внешнего класса.
    pub fn source_for(&self, lib: usize, class_entry: &str, file: Option<&str>) -> Option<(PathBuf, String)> {
        let sources = self.libs.get(lib)?.sources.clone()?;
        let entries = self.source_entries(&sources)?;
        let (dir, simple) = class_entry.strip_suffix(".class")?.rsplit_once('/').unwrap_or(("", class_entry.strip_suffix(".class")?));
        let outer = simple.split('$').next().unwrap_or(simple);
        let names: Vec<String> = match file {
            Some(f) => vec![f.to_owned()],
            None => vec![format!("{outer}.java"), format!("{outer}.kt")],
        };
        names
            .into_iter()
            .map(|n| if dir.is_empty() { n } else { format!("{dir}/{n}") })
            .find(|p| entries.binary_search(p).is_ok())
            .map(|p| (sources, p))
    }

    fn source_entries(&self, jar: &Path) -> Option<Arc<Vec<String>>> {
        let mut cache = self.source_entries.lock().ok()?;
        if let Some(e) = cache.get(jar) {
            return Some(e.clone());
        }
        let entries = Arc::new(list_entries(jar).ok()?);
        cache.insert(jar.to_path_buf(), entries.clone());
        Some(entries)
    }

    /// Сколько jar в группе всего (части и отдельные).
    pub fn group_size(&self, g: &Group) -> usize {
        let mut libs: BTreeSet<usize> = g.parts.iter().map(|p| p.lib).collect();
        libs.extend(g.members.iter().copied());
        libs.len()
    }
}

/// Файлы, которые показываем в дереве: без вложенных классов и служебных `*-info.class`.
fn is_shown_file(name: &str) -> bool {
    if let Some(stem) = name.strip_suffix(".class") {
        return !stem.contains('$') && stem != "package-info" && stem != "module-info";
    }
    true
}

fn is_resource_path(path: &str) -> bool {
    path.starts_with("assets/") || path.starts_with("data/") || path.starts_with("META-INF/")
}

/// Файлы jar по алфавиту.
pub fn list_entries(jar: &Path) -> std::io::Result<Vec<String>> {
    let file = std::fs::File::open(jar)?;
    let archive = zip::ZipArchive::new(std::io::BufReader::new(file)).map_err(std::io::Error::other)?;
    let mut names: Vec<String> = archive.file_names().filter_map(Result::ok).filter(|n| !n.ends_with('/')).map(|n| n.into_owned()).collect();
    names.sort();
    Ok(names)
}

/// Содержимое файла из jar.
pub fn read_entry(jar: &Path, entry: &str) -> std::io::Result<Vec<u8>> {
    let file = std::fs::File::open(jar)?;
    let mut archive = zip::ZipArchive::new(std::io::BufReader::new(file)).map_err(std::io::Error::other)?;
    let mut f = archive.by_name(entry).map_err(std::io::Error::other)?;
    let mut out = Vec::with_capacity(f.size() as usize);
    f.read_to_end(&mut out)?;
    Ok(out)
}

/// Узнаёт, чей jar, и запоминает его файлы. Нечитаемый jar пропускается.
fn inspect(dep: &Dependency, info: &ModInfo) -> Option<Library> {
    if !dep.file.is_file() {
        return None;
    }
    let entries = match list_entries(&dep.file) {
        Ok(e) => e,
        Err(err) => {
            log::warn!("Failed to read {}: {err}", dep.file.display());
            return None;
        }
    };
    let has = |e: &str| entries.binary_search_by(|x| x.as_str().cmp(e)).is_ok();
    let has_prefix = |p: &str| {
        let i = entries.partition_point(|x| x.as_str() < p);
        entries.get(i).is_some_and(|x| x.starts_with(p))
    };
    let file_stem = dep.file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let group = dep.group.clone();
    let g = group.as_deref().unwrap_or("");
    let artifact = dep.name.clone().unwrap_or_else(|| file_stem.clone());

    let mut with_loader = None;
    let mut name = artifact.clone();
    let mut version = dep.version.clone();
    let kind = if has("net/minecraft/client/Minecraft.class") || has("net/minecraft/server/MinecraftServer.class") {
        if has_prefix("net/neoforged/neoforge/") {
            with_loader = Some(Loader::NeoForge);
        } else if has_prefix("net/minecraftforge/") {
            with_loader = Some(Loader::Forge);
        }
        name = "Minecraft".into();
        version = info.minecraft.clone().or(version);
        LibKind::Minecraft
    } else if file_stem.contains("client-extra") && has_prefix("assets/minecraft/") {
        name = "Minecraft".into();
        version = info.minecraft.clone();
        LibKind::Minecraft
    } else if g == "com.mojang" {
        LibKind::Mojang
    } else if g.starts_with("net.neoforged") || g.starts_with("cpw.mods") {
        LibKind::Loader(Loader::NeoForge)
    } else if g.starts_with("net.minecraftforge") {
        LibKind::Loader(Loader::Forge)
    } else if (g == "net.fabricmc" && artifact == "fabric-loader") || g.starts_with("net.fabricmc.fabric-api") {
        LibKind::Loader(Loader::Fabric)
    } else if g.starts_with("org.quiltmc") && artifact.contains("loader") {
        LibKind::Loader(Loader::Quilt)
    } else if let Some((id, mod_name)) = mod_metadata(&dep.file, &has) {
        if id.starts_with("fabric-") || id == "fabricloader" {
            LibKind::Loader(Loader::Fabric)
        } else {
            if let Some(n) = mod_name {
                name = n;
            }
            LibKind::Mod { id }
        }
    } else {
        LibKind::Library
    };
    if version.is_none() && kind == LibKind::Library {
        // Файловая зависимость без координат: версия из имени файла `foo-1.2.3`.
        if let Some((n, v)) = file_stem.rsplit_once('-').filter(|(_, v)| v.starts_with(|c: char| c.is_ascii_digit())) {
            name = n.to_owned();
            version = Some(v.to_owned());
        }
    }
    Some(Library { sources: find_sources(dep), jar: dep.file.clone(), group, name, version, kind, with_loader, entries: Arc::new(entries) })
}

/// Id и имя мода из `fabric.mod.json`, `neoforge.mods.toml` или `mods.toml`.
fn mod_metadata(jar: &Path, has: &dyn Fn(&str) -> bool) -> Option<(String, Option<String>)> {
    if has("fabric.mod.json") {
        let bytes = read_entry(jar, "fabric.mod.json").ok()?;
        let v: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
        let id = v.get("id")?.as_str()?.to_owned();
        let name = v.get("name").and_then(|n| n.as_str()).map(str::to_owned);
        return Some((id, name));
    }
    for meta in ["META-INF/neoforge.mods.toml", "META-INF/mods.toml"] {
        if has(meta) {
            let text = String::from_utf8(read_entry(jar, meta).ok()?).ok()?;
            let id = crate::detect::toml_value(&text, "modId")?;
            let name = crate::detect::toml_value(&text, "displayName");
            return Some((id, name));
        }
    }
    None
}

/// `-sources.jar`: рядом с jar или в соседней папке кеша Gradle (`files-2.1/группа/имя/версия/<хеш>/`).
fn find_sources(dep: &Dependency) -> Option<PathBuf> {
    let stem = dep.file.file_stem()?.to_string_lossy().into_owned();
    let name = format!("{stem}-sources.jar");
    let dir = dep.file.parent()?;
    let sibling = dir.join(&name);
    if sibling.is_file() {
        return Some(sibling);
    }
    if dep.group.is_some() {
        let version_dir = dir.parent()?;
        for hash in std::fs::read_dir(version_dir).ok()?.flatten() {
            let candidate = hash.path().join(&name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn make_groups(libs: &[Library], info: &ModInfo) -> Vec<Group> {
    let mut groups = Vec::new();
    let idx = |pred: &dyn Fn(&Library) -> bool| -> Vec<usize> { libs.iter().enumerate().filter(|(_, l)| pred(l)).map(|(i, _)| i).collect() };

    let minecraft = idx(&|l| l.kind == LibKind::Minecraft);
    if !minecraft.is_empty() {
        groups.push(Group {
            kind: GroupKind::Minecraft,
            version: info.minecraft.clone(),
            parts: minecraft.iter().map(|&lib| Part { lib, prefixes: MINECRAFT_PREFIXES.to_vec() }).collect(),
            members: idx(&|l| l.kind == LibKind::Mojang),
        });
    }

    let mut loaders: BTreeSet<Loader> = libs.iter().filter_map(|l| l.with_loader).collect();
    loaders.extend(libs.iter().filter_map(|l| match l.kind {
        LibKind::Loader(x) => Some(x),
        _ => None,
    }));
    for loader in loaders {
        let prefixes = match loader {
            Loader::NeoForge => NEOFORGE_PREFIXES,
            Loader::Forge => FORGE_PREFIXES,
            Loader::Fabric | Loader::Quilt => &[],
        };
        let version = info.loaders.contains(&loader).then(|| info.loader_version.clone()).flatten();
        groups.push(Group {
            kind: GroupKind::Loader(loader),
            version,
            parts: libs.iter().enumerate().filter(|(_, l)| l.with_loader == Some(loader)).map(|(lib, _)| Part { lib, prefixes: prefixes.to_vec() }).collect(),
            members: idx(&|l| l.kind == LibKind::Loader(loader)),
        });
    }

    let mods = idx(&|l| matches!(l.kind, LibKind::Mod { .. }));
    if !mods.is_empty() {
        groups.push(Group { kind: GroupKind::Mods, version: None, parts: Vec::new(), members: sorted_by_name(libs, mods) });
    }
    let plain = idx(&|l| l.kind == LibKind::Library);
    if !plain.is_empty() {
        groups.push(Group { kind: GroupKind::Libraries, version: None, parts: Vec::new(), members: sorted_by_name(libs, plain) });
    }
    groups
}

fn sorted_by_name(libs: &[Library], mut list: Vec<usize>) -> Vec<usize> {
    list.sort_by(|&a, &b| libs[a].name.to_lowercase().cmp(&libs[b].name.to_lowercase()));
    list
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mycelium-libs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn jar(path: &Path, files: &[(&str, &str)]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        for (name, text) in files {
            zip.start_file(*name, opts).unwrap();
            zip.write_all(text.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }

    fn dep(file: PathBuf, coords: Option<(&str, &str, &str)>) -> Dependency {
        Dependency {
            file,
            group: coords.map(|c| c.0.to_owned()),
            name: coords.map(|c| c.1.to_owned()),
            version: coords.map(|c| c.2.to_owned()),
            project: None,
        }
    }

    #[test]
    fn groups_by_origin_with_merged_minecraft_jar() {
        let root = temp("groups");
        let artifacts = root.join("build/moddev/artifacts");
        jar(
            &artifacts.join("neoforge-21.1.252.jar"),
            &[
                ("net/minecraft/client/Minecraft.class", ""),
                ("net/minecraft/world/item/Item.class", ""),
                ("net/minecraft/world/item/Item$Properties.class", ""),
                ("net/minecraft/world/item/package-info.class", ""),
                ("com/mojang/blaze3d/systems/RenderSystem.class", ""),
                ("net/neoforged/neoforge/common/NeoForge.class", ""),
                ("META-INF/MANIFEST.MF", ""),
            ],
        );
        jar(&artifacts.join("neoforge-21.1.252-sources.jar"), &[("net/minecraft/world/item/Item.java", "class Item {}")]);
        jar(&artifacts.join("neoforge-21.1.252-client-extra-aka-minecraft-resources.jar"), &[("assets/minecraft/models/item/stick.json", "{}")]);
        let cache = root.join("caches/files-2.1");
        jar(&cache.join("com.mojang/brigadier/1.3.10/aaa/brigadier-1.3.10.jar"), &[("com/mojang/brigadier/Command.class", "")]);
        jar(&cache.join("com.mojang/brigadier/1.3.10/bbb/brigadier-1.3.10-sources.jar"), &[("com/mojang/brigadier/Command.java", "")]);
        jar(&cache.join("net.neoforged/bus/8.0.5/ccc/bus-8.0.5.jar"), &[("net/neoforged/bus/api/IEventBus.class", "")]);
        jar(&cache.join("io.github/jei/19/ddd/jei-19.jar"), &[("META-INF/neoforge.mods.toml", "[[mods]]\nmodId=\"jei\"\ndisplayName=\"Just Enough Items\"\n"), ("mezz/jei/Jei.class", "")]);
        jar(&cache.join("com.google.code.gson/gson/2.10.1/eee/gson-2.10.1.jar"), &[("com/google/gson/Gson.class", "")]);

        let model = GradleModel {
            gradle: "9.2.1".into(),
            projects: vec![crate::model::GradleProject {
                dependencies: vec![
                    dep(artifacts.join("neoforge-21.1.252.jar"), None),
                    dep(cache.join("com.mojang/brigadier/1.3.10/aaa/brigadier-1.3.10.jar"), Some(("com.mojang", "brigadier", "1.3.10"))),
                    dep(cache.join("net.neoforged/bus/8.0.5/ccc/bus-8.0.5.jar"), Some(("net.neoforged", "bus", "8.0.5"))),
                    dep(cache.join("io.github/jei/19/ddd/jei-19.jar"), Some(("io.github", "jei", "19"))),
                    dep(cache.join("com.google.code.gson/gson/2.10.1/eee/gson-2.10.1.jar"), Some(("com.google.code.gson", "gson", "2.10.1"))),
                ],
                ..Default::default()
            }],
        };
        let info = ModInfo { minecraft: Some("1.21.1".into()), loaders: vec![Loader::NeoForge], loader_version: Some("21.1.252".into()), ..ModInfo::default() };
        let libs = Libraries::build(&model, &info);

        let kinds: Vec<GroupKind> = libs.groups.iter().map(|g| g.kind.clone()).collect();
        assert_eq!(kinds, vec![GroupKind::Minecraft, GroupKind::Loader(Loader::NeoForge), GroupKind::Mods, GroupKind::Libraries]);
        let mc = &libs.groups[0];
        assert_eq!(mc.parts.len(), 2, "классы и отдельный jar ресурсов");
        assert_eq!(mc.members.len(), 1, "brigadier — библиотека Mojang");
        assert_eq!(libs.libs[libs.groups[2].members[0]].name, "Just Enough Items");

        // Корень Minecraft: цепочки свёрнуты, NeoForge и META-INF не видны.
        let names: Vec<String> = libs.children(&mc.parts, "").into_iter().map(|n| n.name).collect();
        assert_eq!(names, vec!["assets/minecraft/models/item", "com.mojang.blaze3d.systems", "net.minecraft"]);
        let items = libs.children(&mc.parts, "net/minecraft/world/item/");
        assert_eq!(items.iter().map(|n| n.name.as_str()).collect::<Vec<_>>(), vec!["Item"], "без вложенных классов и package-info");
        assert!(items[0].is_class());
        let neo = &libs.groups[1];
        let names: Vec<String> = libs.children(&neo.parts, "").into_iter().map(|n| n.name).collect();
        assert_eq!(names, vec!["net.neoforged.neoforge.common"]);

        // Класс → исходник рядом с jar, библиотека → исходник в соседней папке кеша.
        let (lib, entry) = libs.find_class("net.minecraft.world.item.Item$Properties").unwrap();
        assert_eq!(entry, "net/minecraft/world/item/Item.class");
        let (src, path) = libs.source_for(lib, &entry, None).unwrap();
        assert!(src.ends_with("neoforge-21.1.252-sources.jar"));
        assert_eq!(path, "net/minecraft/world/item/Item.java");
        assert_eq!(read_entry(&src, &path).unwrap(), b"class Item {}");
        let (lib, entry) = libs.find_class("com.mojang.brigadier.Command").unwrap();
        assert!(libs.source_for(lib, &entry, Some("Command.java")).is_some());

        // Вложенный класс есть, а класса из старого пакета нет — нашёлся тот же в новом.
        assert!(libs.has_class("net.minecraft.world.item.Item$Properties"));
        assert!(libs.has_class("net/minecraft/world/item/Item"));
        assert!(!libs.has_class("net.minecraft.world.item.Item$Missing"));
        assert!(!libs.has_class("net.minecraft.item.Item$Properties"));
        assert_eq!(libs.classes_named("net.minecraft.item.Item$Properties"), vec!["net.minecraft.world.item.Item$Properties"]);
        assert!(libs.classes_named("net.minecraft.world.item.Item").is_empty(), "сам класс — не «похожий»");
        let _ = std::fs::remove_dir_all(&root);
    }
}
