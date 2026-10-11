//! Распознавание мода по файлам проекта: загрузчик (Fabric, NeoForge, Forge, Quilt),
//! версии Minecraft и загрузчика, id и имя мода, языки, исходники и пакеты мода.
//!
//! Gradle здесь не запускается — только чтение файлов, поэтому это быстро и работает
//! даже для проекта, который ещё ни разу не собирался.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Loader {
    Fabric,
    NeoForge,
    Forge,
    Quilt,
}

impl Loader {
    pub fn name(self) -> &'static str {
        match self {
            Loader::Fabric => "Fabric",
            Loader::NeoForge => "NeoForge",
            Loader::Forge => "Forge",
            Loader::Quilt => "Quilt",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Language {
    Java,
    Kotlin,
}

/// Модуль Gradle-проекта (корень или подпроект вроде `common`, `fabric`, `neoforge`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Module {
    /// Путь относительно корня; пустой — сам корень.
    pub path: PathBuf,
    pub loader: Option<Loader>,
    pub source_roots: Vec<PathBuf>,
    pub resource_roots: Vec<PathBuf>,
}

/// Что удалось узнать о моде.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModInfo {
    /// Найден Gradle-проект (есть build.gradle или settings.gradle).
    pub gradle: bool,
    /// Есть обёртка `gradlew` — запускаем через неё.
    pub wrapper: bool,
    pub loaders: Vec<Loader>,
    pub minecraft: Option<String>,
    /// Версия загрузчика: `neo_version`, `loader_version` у Fabric, `forge_version`.
    pub loader_version: Option<String>,
    pub mod_id: Option<String>,
    pub mod_name: Option<String>,
    pub mod_version: Option<String>,
    pub languages: Vec<Language>,
    pub modules: Vec<Module>,
    /// Корневые пакеты кода мода (`com.example.mymod`) — по ним отличаем свой код от чужого.
    pub packages: Vec<String>,
    /// Иконка мода (`logoFile` или `icon` из метаданных) — путь от корня проекта.
    pub icon: Option<PathBuf>,
}

impl ModInfo {
    /// Похоже ли это на мод (найден хотя бы один загрузчик).
    pub fn is_mod(&self) -> bool {
        !self.loaders.is_empty()
    }

    /// Несколько загрузчиков сразу — мультилоадерный проект (common + fabric + neoforge).
    pub fn is_multiloader(&self) -> bool {
        self.loaders.len() > 1
    }

    pub fn source_roots(&self) -> impl Iterator<Item = &PathBuf> {
        self.modules.iter().flat_map(|m| m.source_roots.iter())
    }

    pub fn resource_roots(&self) -> impl Iterator<Item = &PathBuf> {
        self.modules.iter().flat_map(|m| m.resource_roots.iter())
    }
}

/// Распознаёт мод в папке `root`.
pub fn detect(root: &Path) -> ModInfo {
    let props = read_properties(&root.join("gradle.properties"));
    let settings = read_first(root, &["settings.gradle", "settings.gradle.kts"]).unwrap_or_default();
    let mut info = ModInfo {
        gradle: ["build.gradle", "build.gradle.kts", "settings.gradle", "settings.gradle.kts"].iter().any(|f| root.join(f).is_file()),
        wrapper: root.join("gradlew").is_file() || root.join("gradlew.bat").is_file(),
        ..ModInfo::default()
    };

    // Корень и подпроекты из settings.gradle.
    let mut module_dirs = vec![PathBuf::new()];
    for name in included_projects(&settings) {
        let rel = PathBuf::from(name.trim_start_matches(':').replace(':', "/"));
        if root.join(&rel).is_dir() && !module_dirs.contains(&rel) {
            module_dirs.push(rel);
        }
    }

    let mut loaders = BTreeSet::new();
    let mut languages = BTreeSet::new();
    let mut fabric_meta: Option<serde_json::Value> = None;
    let mut toml_meta: Option<String> = None;
    for rel in module_dirs {
        let dir = root.join(&rel);
        let build = read_first(&dir, &["build.gradle", "build.gradle.kts"]).unwrap_or_default();
        let resources = dir.join("src/main/resources");
        let fabric_json = resources.join("fabric.mod.json");
        let neo_toml = [resources.join("META-INF/neoforge.mods.toml"), dir.join("src/main/templates/META-INF/neoforge.mods.toml")]
            .into_iter()
            .find(|p| p.is_file());
        let forge_toml = resources.join("META-INF/mods.toml");

        let loader = if fabric_json.is_file() || build.contains("fabric-loom") || build.contains("net.fabricmc") {
            Some(Loader::Fabric)
        } else if neo_toml.is_some() || build.contains("net.neoforged") {
            Some(Loader::NeoForge)
        } else if resources.join("quilt.mod.json").is_file() || build.contains("quilt-loom") {
            Some(Loader::Quilt)
        } else if forge_toml.is_file() || build.contains("net.minecraftforge") {
            Some(Loader::Forge)
        } else {
            None
        };
        if let Some(l) = loader {
            loaders.insert(l);
        }
        if fabric_meta.is_none() {
            fabric_meta = std::fs::read_to_string(&fabric_json).ok().and_then(|s| serde_json::from_str(&s).ok());
        }
        if toml_meta.is_none() {
            toml_meta = neo_toml.or_else(|| forge_toml.is_file().then_some(forge_toml)).and_then(|p| std::fs::read_to_string(p).ok());
        }

        let mut module = Module { path: rel.clone(), loader, ..Module::default() };
        // `src/client/*` — раздельные наборы исходников клиента у Fabric Loom.
        let sources = [
            (Language::Java, "src/main/java"),
            (Language::Kotlin, "src/main/kotlin"),
            (Language::Java, "src/client/java"),
            (Language::Kotlin, "src/client/kotlin"),
        ];
        for (lang, sub) in sources {
            if dir.join(sub).is_dir() {
                languages.insert(lang);
                module.source_roots.push(rel.join(sub));
            }
        }
        if build.contains("kotlin") {
            languages.insert(Language::Kotlin);
        }
        for sub in ["src/main/resources", "src/client/resources", "src/generated/resources", "src/main/generated"] {
            if dir.join(sub).is_dir() {
                module.resource_roots.push(rel.join(sub));
            }
        }
        if loader.is_some() || !module.source_roots.is_empty() {
            info.modules.push(module);
        }
    }
    info.loaders = loaders.into_iter().collect();
    info.languages = languages.into_iter().collect();

    // Версии и id мода: сначала gradle.properties, потом метаданные мода.
    let prop = |keys: &[&str]| keys.iter().find_map(|k| props.get(*k).filter(|v| !v.is_empty()).cloned());
    info.minecraft = prop(&["minecraft_version", "mc_version"]);
    info.loader_version = prop(&["neo_version", "neoforge_version", "loader_version", "fabric_loader_version", "forge_version"]);
    info.mod_id = prop(&["mod_id", "modid"]);
    info.mod_name = prop(&["mod_name"]);
    info.mod_version = prop(&["mod_version", "version"]);

    let resolve = |s: &str| expand_placeholders(s, &props);
    if let Some(meta) = &fabric_meta {
        let field = |k: &str| meta.get(k).and_then(|v| v.as_str()).map(|s| resolve(s)).filter(|s| !s.contains("${"));
        info.mod_id = info.mod_id.take().or_else(|| field("id"));
        info.mod_name = info.mod_name.take().or_else(|| field("name"));
        if info.minecraft.is_none() {
            info.minecraft = meta
                .get("depends")
                .and_then(|d| d.get("minecraft"))
                .and_then(|v| v.as_str())
                .map(|s| resolve(s).trim_start_matches(['~', '^', '>', '=', '<']).to_owned())
                .filter(|s| !s.is_empty() && !s.contains("${") && s != "*");
        }
    }
    if let Some(toml) = &toml_meta {
        info.mod_id = info.mod_id.take().or_else(|| toml_value(toml, "modId").map(|s| resolve(&s)).filter(|s| !s.contains("${")));
        info.mod_name = info.mod_name.take().or_else(|| toml_value(toml, "displayName").map(|s| resolve(&s)).filter(|s| !s.contains("${")));
    }

    // Иконка: `icon` у Fabric (строка или размеры → самый большой), `logoFile` у NeoForge и Forge.
    // Путь в метаданных — от корня jar, то есть от папки ресурсов.
    let icon = fabric_meta
        .as_ref()
        .and_then(|m| m.get("icon"))
        .and_then(|v| match v {
            serde_json::Value::String(s) => Some(s.clone()),
            serde_json::Value::Object(o) => o.iter().max_by_key(|(k, _)| k.parse::<u32>().unwrap_or(0)).and_then(|(_, v)| v.as_str().map(str::to_owned)),
            _ => None,
        })
        .or_else(|| toml_meta.as_ref().and_then(|t| toml_value(t, "logoFile")))
        .map(|s| resolve(&s));
    info.icon = icon.and_then(|rel| info.resource_roots().map(|r| r.join(rel.trim_start_matches('/'))).find(|p| root.join(p).is_file()));

    // Последняя надежда — имя архива из Fabric-шаблона.
    info.mod_id = info.mod_id.take().or_else(|| prop(&["archives_base_name"]));
    info.packages = base_packages(root, info.source_roots());
    info
}

/// Читает `gradle.properties`: `ключ=значение`, комментарии с `#` и `!`.
fn read_properties(path: &Path) -> BTreeMap<String, String> {
    let Ok(text) = std::fs::read_to_string(path) else { return BTreeMap::new() };
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('!'))
        .filter_map(|l| {
            let i = l.find(['=', ':'])?;
            Some((l[..i].trim().to_owned(), l[i + 1..].trim().to_owned()))
        })
        .collect()
}

fn read_first(dir: &Path, names: &[&str]) -> Option<String> {
    names.iter().find_map(|n| std::fs::read_to_string(dir.join(n)).ok())
}

/// Подпроекты из `include 'a', ':b'` или `include("a", "b")`.
fn included_projects(settings: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in settings.lines() {
        let line = line.trim();
        if !line.starts_with("include") || line.starts_with("includeBuild") {
            continue;
        }
        let mut rest = &line["include".len()..];
        while let Some(start) = rest.find(['"', '\'']) {
            let quote = rest.as_bytes()[start] as char;
            let after = &rest[start + 1..];
            let Some(end) = after.find(quote) else { break };
            let name = &after[..end];
            if !name.is_empty() {
                out.push(name.to_owned());
            }
            rest = &after[end + 1..];
        }
    }
    out
}

/// Подставляет `${ключ}` из gradle.properties (так выглядят шаблоны метаданных мода).
fn expand_placeholders(s: &str, props: &BTreeMap<String, String>) -> String {
    let mut out = s.to_owned();
    for (k, v) in props {
        out = out.replace(&format!("${{{k}}}"), v);
    }
    out
}

/// Значение `ключ = "значение"` из TOML (нам хватает простого разбора строк).
pub(crate) fn toml_value(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let line = line.trim();
        let rest = line.strip_prefix(key)?.trim_start();
        let rest = rest.strip_prefix('=')?.trim();
        let quote = rest.chars().next().filter(|c| *c == '"' || *c == '\'')?;
        let inner = &rest[1..];
        inner.find(quote).map(|end| inner[..end].to_owned())
    })
}

/// Корневые пакеты мода: общий префикс пакетов всех исходников. Если общего префикса
/// нет (например, `com.a.x` и `net.b.y`), — по два первых сегмента каждого.
fn base_packages<'a>(root: &Path, roots: impl Iterator<Item = &'a PathBuf>) -> Vec<String> {
    let mut packages: BTreeSet<Vec<String>> = BTreeSet::new();
    for src in roots {
        collect_packages(&root.join(src), &mut Vec::new(), &mut packages, 0);
    }
    if packages.is_empty() {
        return Vec::new();
    }
    let all: Vec<&Vec<String>> = packages.iter().collect();
    let mut common: Vec<String> = all[0].clone();
    for p in &all[1..] {
        let n = common.iter().zip(p.iter()).take_while(|(a, b)| a == b).count();
        common.truncate(n);
    }
    if common.len() >= 2 {
        return vec![common.join(".")];
    }
    let mut groups: BTreeSet<String> = BTreeSet::new();
    for p in all {
        groups.insert(p.iter().take(2).cloned().collect::<Vec<_>>().join("."));
    }
    groups.into_iter().filter(|g| !g.is_empty()).collect()
}

fn collect_packages(dir: &Path, prefix: &mut Vec<String>, out: &mut BTreeSet<Vec<String>>, depth: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    if depth > 12 {
        return;
    }
    let mut has_sources = false;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            prefix.push(name);
            collect_packages(&path, prefix, out, depth + 1);
            prefix.pop();
        } else if name.ends_with(".java") || name.ends_with(".kt") {
            has_sources = true;
        }
    }
    if has_sources && !prefix.is_empty() {
        out.insert(prefix.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mycelium-detect-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(root: &Path, rel: &str, text: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn neoforge_mdk() {
        let root = temp("neo");
        write(&root, "build.gradle", "plugins {\n    id 'net.neoforged.moddev' version '2.0.148'\n}\n");
        write(&root, "settings.gradle", "pluginManagement { }\n");
        write(&root, "gradlew.bat", "");
        write(
            &root,
            "gradle.properties",
            "# комментарий\nminecraft_version=1.21.1\nneo_version=21.1.252\nmod_id=testmod\nmod_name=Test Mod\nmod_version=1.0.0\n",
        );
        write(&root, "src/main/templates/META-INF/neoforge.mods.toml", "[[mods]]\nmodId=\"${mod_id}\"\ndisplayName=\"${mod_name}\"\nlogoFile=\"${mod_id}.png\"\n");
        write(&root, "src/main/resources/testmod.png", "png");
        write(&root, "src/main/java/com/holo/testmod/TestMod.java", "package com.holo.testmod;");
        write(&root, "src/main/java/com/holo/testmod/item/Ruby.java", "package com.holo.testmod.item;");
        let info = detect(&root);
        assert!(info.gradle && info.wrapper);
        assert_eq!(info.loaders, vec![Loader::NeoForge]);
        assert_eq!(info.minecraft.as_deref(), Some("1.21.1"));
        assert_eq!(info.loader_version.as_deref(), Some("21.1.252"));
        assert_eq!(info.mod_id.as_deref(), Some("testmod"));
        assert_eq!(info.mod_name.as_deref(), Some("Test Mod"));
        assert_eq!(info.languages, vec![Language::Java]);
        assert_eq!(info.packages, vec!["com.holo.testmod".to_owned()]);
        assert_eq!(info.icon, Some(PathBuf::from("src/main/resources/testmod.png")), "logoFile с подстановкой из gradle.properties");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn fabric_example_mod() {
        let root = temp("fabric");
        write(&root, "build.gradle", "plugins {\n id 'fabric-loom' version '1.11-SNAPSHOT'\n}\n");
        write(&root, "gradle.properties", "minecraft_version=1.21.1\nloader_version=0.16.5\narchives_base_name=ruby-mod\n");
        write(&root, "src/main/resources/fabric.mod.json", r#"{"id": "rubymod", "name": "Ruby", "icon": {"16": "assets/rubymod/small.png", "128": "assets/rubymod/icon.png"}, "depends": {"minecraft": "~1.21.1"}}"#);
        write(&root, "src/main/resources/assets/rubymod/icon.png", "png");
        write(&root, "src/main/kotlin/net/holo/ruby/Ruby.kt", "package net.holo.ruby");
        write(&root, "src/client/java/net/holo/ruby/client/RubyClient.java", "");
        let info = detect(&root);
        assert_eq!(info.loaders, vec![Loader::Fabric]);
        assert_eq!(info.loader_version.as_deref(), Some("0.16.5"));
        assert_eq!(info.mod_id.as_deref(), Some("rubymod"), "id из fabric.mod.json важнее имени архива");
        assert_eq!(info.mod_name.as_deref(), Some("Ruby"));
        assert_eq!(info.languages, vec![Language::Java, Language::Kotlin]);
        assert_eq!(info.packages, vec!["net.holo.ruby".to_owned()]);
        assert_eq!(info.modules[0].source_roots.len(), 2, "и main, и client");
        assert_eq!(info.icon, Some(PathBuf::from("src/main/resources/assets/rubymod/icon.png")), "самый большой размер");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn multiloader_with_subprojects() {
        let root = temp("multi");
        write(&root, "settings.gradle", "include(\"common\")\ninclude 'fabric', ':neoforge'\n");
        write(&root, "gradle.properties", "minecraft_version=1.21.1\nmod_id=multi\n");
        write(&root, "common/build.gradle", "");
        write(&root, "common/src/main/java/com/holo/multi/Common.java", "");
        write(&root, "fabric/build.gradle", "plugins { id 'fabric-loom' }");
        write(&root, "fabric/src/main/java/com/holo/multi/fabric/F.java", "");
        write(&root, "neoforge/build.gradle", "plugins { id 'net.neoforged.moddev' }");
        write(&root, "neoforge/src/main/java/com/holo/multi/neoforge/N.java", "");
        let info = detect(&root);
        assert!(info.is_multiloader());
        assert_eq!(info.loaders, vec![Loader::Fabric, Loader::NeoForge]);
        assert_eq!(info.modules.len(), 3);
        assert_eq!(info.packages, vec!["com.holo.multi".to_owned()]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn plain_folder_is_not_a_mod() {
        let root = temp("plain");
        write(&root, "notes.txt", "");
        let info = detect(&root);
        assert!(!info.is_mod() && !info.gradle);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn scattered_packages_split_by_two_segments() {
        let root = temp("pkgs");
        write(&root, "src/main/java/com/a/x/A.java", "");
        write(&root, "src/main/java/net/b/y/B.java", "");
        let info = detect(&root);
        assert_eq!(info.packages, vec!["com.a".to_owned(), "net.b".to_owned()]);
        let _ = std::fs::remove_dir_all(&root);
    }
}
