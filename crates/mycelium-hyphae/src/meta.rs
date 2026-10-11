//! Метаданные модов: `META-INF/neoforge.mods.toml` (NeoForge), `META-INF/mods.toml`
//! (Forge), `fabric.mod.json` (Fabric) и `quilt.mod.json` (Quilt). В одном jar бывает
//! несколько модов. Отсюда же — какие jar лежат внутри (jar-in-jar).

use serde_json::Value;

use crate::jar::Jar;

/// Формат метаданных — по нему понятно, для какого загрузчика jar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Format {
    NeoForge,
    Forge,
    Fabric,
    Quilt,
}

/// Зависимость мода от другого мода.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModDependency {
    pub id: String,
    pub required: bool,
    /// Диапазон версий как в метаданных: `[6.0.7,)`, `>=0.15`.
    pub versions: String,
}

/// Один мод из метаданных.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModMeta {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub authors: String,
    /// Путь к логотипу внутри jar.
    pub logo: Option<String>,
    pub dependencies: Vec<ModDependency>,
}

/// Что jar рассказывает о себе.
#[derive(Clone, Debug, PartialEq)]
pub struct JarMeta {
    pub format: Format,
    pub mods: Vec<ModMeta>,
    /// Конфигурации миксинов (`railways.mixins.json`).
    pub mixins: Vec<String>,
    /// Вложенные jar (пути внутри).
    pub nested: Vec<String>,
}

/// Читает метаданные jar. Не мод — `None`; вложенные jar находятся и у не-модов
/// (у самого NeoForge, например).
pub fn read(jar: &mut Jar) -> Option<JarMeta> {
    let nested = nested_jars(jar);
    if let Some(text) = jar.read_text("META-INF/neoforge.mods.toml") {
        let version = jar_version(jar);
        return Some(from_mods_toml(&text, Format::NeoForge, version.as_deref(), nested));
    }
    if let Some(text) = jar.read_text("META-INF/mods.toml") {
        let version = jar_version(jar);
        return Some(from_mods_toml(&text, Format::Forge, version.as_deref(), nested));
    }
    if let Some(text) = jar.read_text("fabric.mod.json") {
        return from_fabric(&text, nested);
    }
    if let Some(text) = jar.read_text("quilt.mod.json") {
        return from_quilt(&text, nested);
    }
    None
}

/// `Implementation-Version` манифеста — подставляется вместо `${file.jarVersion}`.
fn jar_version(jar: &mut Jar) -> Option<String> {
    jar.manifest().remove("Implementation-Version")
}

/// Вложенные jar: NeoForge и Forge перечисляют их в `META-INF/jarjar/metadata.json`,
/// Fabric — в `jars` метаданных, Quilt — в `quilt_loader.jars`.
fn nested_jars(jar: &mut Jar) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(v) = jar.read_text("META-INF/jarjar/metadata.json").and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
        for j in v.get("jars").and_then(Value::as_array).into_iter().flatten() {
            if let Some(p) = j.get("path").and_then(Value::as_str) {
                out.push(p.to_owned());
            }
        }
    }
    if let Some(v) = jar.read_text("fabric.mod.json").and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
        for j in v.get("jars").and_then(Value::as_array).into_iter().flatten() {
            if let Some(p) = j.get("file").and_then(Value::as_str) {
                out.push(p.to_owned());
            }
        }
    }
    if let Some(v) = jar.read_text("quilt.mod.json").and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
        for j in v.pointer("/quilt_loader/jars").and_then(Value::as_array).into_iter().flatten() {
            if let Some(p) = j.as_str() {
                out.push(p.to_owned());
            }
        }
    }
    out.retain(|p| jar.has(p));
    out.dedup();
    out
}

/// Строковое значение TOML или из массива строк (`authors` бывает и так, и так).
fn toml_text(v: Option<&toml::Value>) -> String {
    match v {
        Some(toml::Value::String(s)) => s.trim().to_owned(),
        Some(toml::Value::Array(a)) => a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(", "),
        _ => String::new(),
    }
}

/// `neoforge.mods.toml` / `mods.toml`. Испорченный TOML читается по строкам: хотя бы id и имя.
pub fn from_mods_toml(text: &str, format: Format, jar_version: Option<&str>, nested: Vec<String>) -> JarMeta {
    let Ok(doc) = toml::from_str::<toml::Table>(text) else {
        let id = mycelium_project::detect::toml_value(text, "modId").unwrap_or_default();
        let name = mycelium_project::detect::toml_value(text, "displayName").unwrap_or_else(|| id.clone());
        let mods = if id.is_empty() { Vec::new() } else { vec![ModMeta { id, name, ..ModMeta::default() }] };
        return JarMeta { format, mods, mixins: Vec::new(), nested };
    };
    let top_logo = doc.get("logoFile").and_then(|v| v.as_str()).map(str::to_owned);
    let deps = doc.get("dependencies").and_then(|v| v.as_table());
    let mut mods = Vec::new();
    for m in doc.get("mods").and_then(|v| v.as_array()).into_iter().flatten() {
        let Some(id) = m.get("modId").and_then(|v| v.as_str()) else { continue };
        let mut version = toml_text(m.get("version"));
        if version.contains("${file.jarVersion}") {
            version = jar_version.map(str::to_owned).unwrap_or_default();
        }
        let dependencies = deps
            .and_then(|d| d.get(id))
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
            .filter_map(|d| {
                let dep = d.get("modId")?.as_str()?.to_owned();
                // NeoForge: `type = "required"`; Forge и старые NeoForge: `mandatory = true`.
                let required = match d.get("type").and_then(|v| v.as_str()) {
                    Some(t) => t.eq_ignore_ascii_case("required"),
                    None => d.get("mandatory").and_then(|v| v.as_bool()).unwrap_or(false),
                };
                Some(ModDependency { id: dep, required, versions: toml_text(d.get("versionRange")) })
            })
            .collect();
        mods.push(ModMeta {
            id: id.to_owned(),
            name: Some(toml_text(m.get("displayName"))).filter(|n| !n.is_empty()).unwrap_or_else(|| id.to_owned()),
            version,
            description: toml_text(m.get("description")),
            authors: toml_text(m.get("authors")),
            logo: m.get("logoFile").and_then(|v| v.as_str()).map(str::to_owned).or_else(|| top_logo.clone()),
            dependencies,
        });
    }
    let mixins = doc
        .get("mixins")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|m| m.get("config").and_then(|c| c.as_str()).map(str::to_owned))
        .collect();
    JarMeta { format, mods, mixins, nested }
}

/// Строка или массив строк JSON одной строкой.
fn json_text(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.trim().to_owned(),
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|x| x.as_str().map(str::to_owned).or_else(|| x.get("name").and_then(Value::as_str).map(str::to_owned)))
            .collect::<Vec<_>>()
            .join(", "),
        _ => String::new(),
    }
}

/// Значок Fabric: строка или словарь «размер → путь» (берём самый большой).
fn fabric_icon(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) => Some(s.clone()),
        Value::Object(map) => map.iter().max_by_key(|(size, _)| size.parse::<u32>().unwrap_or(0)).and_then(|(_, p)| p.as_str().map(str::to_owned)),
        _ => None,
    }
}

/// Зависимости Fabric: `{"create": ">=6.0"}`; значение бывает и массивом диапазонов.
fn fabric_deps(v: Option<&Value>, required: bool) -> Vec<ModDependency> {
    v.and_then(Value::as_object)
        .into_iter()
        .flatten()
        .map(|(id, range)| ModDependency { id: id.clone(), required, versions: json_text(Some(range)) })
        .collect()
}

pub fn from_fabric(text: &str, nested: Vec<String>) -> Option<JarMeta> {
    let v: Value = serde_json::from_str(text).ok()?;
    let id = v.get("id")?.as_str()?.to_owned();
    let mut dependencies = fabric_deps(v.get("depends"), true);
    dependencies.extend(fabric_deps(v.get("recommends"), false));
    let m = ModMeta {
        name: Some(json_text(v.get("name"))).filter(|n| !n.is_empty()).unwrap_or_else(|| id.clone()),
        id,
        version: json_text(v.get("version")),
        description: json_text(v.get("description")),
        authors: json_text(v.get("authors")),
        logo: fabric_icon(v.get("icon")),
        dependencies,
    };
    let mixins = v
        .get("mixins")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|m| m.as_str().map(str::to_owned).or_else(|| m.get("config").and_then(Value::as_str).map(str::to_owned)))
        .collect();
    Some(JarMeta { format: Format::Fabric, mods: vec![m], mixins, nested })
}

pub fn from_quilt(text: &str, nested: Vec<String>) -> Option<JarMeta> {
    let v: Value = serde_json::from_str(text).ok()?;
    let loader = v.get("quilt_loader")?;
    let id = loader.get("id")?.as_str()?.to_owned();
    let meta = loader.get("metadata");
    let dependencies = loader
        .get("depends")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|d| match d {
            Value::String(s) => Some(ModDependency { id: s.clone(), required: true, versions: String::new() }),
            Value::Object(_) => Some(ModDependency {
                id: d.get("id")?.as_str()?.to_owned(),
                required: !d.get("optional").and_then(Value::as_bool).unwrap_or(false),
                versions: json_text(d.get("versions")),
            }),
            _ => None,
        })
        .collect();
    let m = ModMeta {
        name: Some(json_text(meta.and_then(|m| m.get("name")))).filter(|n| !n.is_empty()).unwrap_or_else(|| id.clone()),
        id,
        version: json_text(loader.get("version")),
        description: json_text(meta.and_then(|m| m.get("description"))),
        authors: meta.and_then(|m| m.get("contributors")).and_then(Value::as_object).map(|c| c.keys().cloned().collect::<Vec<_>>().join(", ")).unwrap_or_default(),
        logo: fabric_icon(meta.and_then(|m| m.get("icon"))),
        dependencies,
    };
    let mixins = match v.get("mixin") {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(a)) => a.iter().filter_map(|m| m.as_str().map(str::to_owned)).collect(),
        _ => Vec::new(),
    };
    Some(JarMeta { format: Format::Quilt, mods: vec![m], mixins, nested })
}

#[cfg(test)]
mod tests {
    use super::*;

    const RAILWAYS: &str = r#"
modLoader = "javafml"
loaderVersion = "[4,)"
license = "LGPL-3.0 license"

[[mods]]
modId = "railways"
version = "${file.jarVersion}"
displayName = "Create: Steam 'n' Rails 1.21.1"
authors = "PoppyBlossom, Chameleon538"
description = '''Unofficial port.'''
logoFile = "icon.png"

[[mixins]]
config = "railways-common.mixins.json"

[[mixins]]
config = "railways.mixins.json"

[[dependencies.railways]]
modId = "create"
mandatory = true
versionRange = "[6.0.7,)"

[[dependencies.railways]]
modId = "hexcasting"
type = "optional"
versionRange = "[0,)"
"#;

    #[test]
    fn neoforge_metadata() {
        let m = from_mods_toml(RAILWAYS, Format::NeoForge, Some("0.3.0-beta.2+neoforge-mc1.21.1"), Vec::new());
        assert_eq!(m.mods.len(), 1);
        let r = &m.mods[0];
        assert_eq!((r.id.as_str(), r.name.as_str()), ("railways", "Create: Steam 'n' Rails 1.21.1"));
        assert_eq!(r.version, "0.3.0-beta.2+neoforge-mc1.21.1", "версия из манифеста");
        assert_eq!(r.logo.as_deref(), Some("icon.png"));
        assert_eq!(m.mixins, vec!["railways-common.mixins.json", "railways.mixins.json"]);
        assert_eq!(r.dependencies.iter().map(|d| (d.id.as_str(), d.required)).collect::<Vec<_>>(), vec![("create", true), ("hexcasting", false)]);
    }

    #[test]
    fn broken_toml_still_gives_the_id() {
        let m = from_mods_toml("[[mods]]\nmodId=\"testmod\"\ndisplayName=\"Test\"\nversion=${broken", Format::NeoForge, None, Vec::new());
        assert_eq!(m.mods[0].id, "testmod");
        assert_eq!(m.mods[0].name, "Test");
    }

    #[test]
    fn fabric_metadata() {
        let text = r#"{"schemaVersion":1,"id":"sodium","version":"0.6.0","name":"Sodium","authors":["jellysquid3",{"name":"IMS"}],
            "icon":{"16":"a.png","128":"assets/sodium/icon.png"},"depends":{"minecraft":">=1.21"},"mixins":["sodium.mixins.json",{"config":"sodium-client.mixins.json"}],
            "jars":[{"file":"META-INF/jars/lib.jar"}]}"#;
        let m = from_fabric(text, Vec::new()).unwrap();
        assert_eq!(m.mods[0].authors, "jellysquid3, IMS");
        assert_eq!(m.mods[0].logo.as_deref(), Some("assets/sodium/icon.png"), "самый крупный значок");
        assert_eq!(m.mixins.len(), 2);
        assert_eq!(m.mods[0].dependencies[0].id, "minecraft");
    }
}
