//! Происхождение кода и ресурсов: мой мод, Minecraft, загрузчик, другой мод, библиотека, Java.
//! Одна и та же палитра происхождения используется везде — в дереве, консоли, исключениях
//! и (позже) на карте мода.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::detect::{Loader, ModInfo};

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Origin {
    /// Код и ресурсы открытого мода.
    OwnMod,
    Minecraft,
    NeoForge,
    Fabric,
    Forge,
    /// Другой мод (id, если известен).
    OtherMod(Option<String>),
    Library,
    /// JDK: `java.*`, `jdk.*`, `sun.*`.
    Java,
    Unknown,
}

impl Origin {
    /// Код загрузчика (NeoForge, Fabric, Forge).
    pub fn is_loader(&self) -> bool {
        matches!(self, Origin::NeoForge | Origin::Fabric | Origin::Forge)
    }
}

/// Что знаем о своём моде, чтобы отличать его код от чужого, и о модах запуска.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OriginRules {
    pub mod_ids: Vec<String>,
    pub packages: Vec<String>,
    /// Пакеты и модули модов запуска (их разбирает Hyphae). Пусто, пока моды не разобраны.
    pub mods: Arc<ModMap>,
}

/// Чьи пакеты и модули JPMS в запуске: так класс `com.simibubi.create.Foo` — это мод
/// Create, а не просто «библиотека».
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModMap {
    /// Пакет через точку (`com.simibubi.create.content`) → происхождение.
    pub packages: HashMap<String, Origin>,
    /// Имя модуля JPMS без версии (`create`, `com.google.gson`) → происхождение.
    pub modules: HashMap<String, Origin>,
    /// Пространство имён, которое не id мода, но которое кладёт в свои ресурсы ровно один
    /// мод запуска (`byg` в jar Steam 'n' Rails) → этот мод.
    pub namespaces: HashMap<String, Origin>,
    /// Конфигурация миксинов (`railways-common.mixins.json`) → мод.
    pub mixins: HashMap<String, Origin>,
    /// Сокращённый пакет из консоли NeoForge (`co.si.cr.co`) → полный пакет.
    pub abbreviated: HashMap<String, String>,
}

/// Пакет, сокращённый, как в консоли NeoForge (`%c{2.}`): у каждой части — первые две
/// буквы (`com.simibubi.create` → `co.si.cr`).
pub fn abbreviate_package(package: &str) -> String {
    package.split('.').map(|s| s.chars().take(2).collect::<String>()).collect::<Vec<_>>().join(".")
}

/// Пакеты загрузчиков и библиотек, по которым разворачиваются сокращённые логгеры.
const KNOWN_PACKAGES: &[&str] = &[
    "net.minecraft",
    "com.mojang",
    "net.neoforged.fml",
    "net.neoforged.fml.loading",
    "net.neoforged.neoforge",
    "net.neoforged.neoforgespi",
    "net.neoforged.bus",
    "cpw.mods.modlauncher",
    "cpw.mods.bootstraplauncher",
    "org.spongepowered.asm.mixin",
    "net.fabricmc.loader",
    "net.minecraftforge.fml",
];

/// Пакеты Minecraft (включая библиотеки Mojang, которые идут с игрой).
const MINECRAFT: &[&str] = &["net.minecraft.", "com.mojang."];
const NEOFORGE: &[&str] = &["net.neoforged.", "cpw.mods."];
const FABRIC: &[&str] = &["net.fabricmc."];
const FORGE: &[&str] = &["net.minecraftforge."];
const JAVA: &[&str] = &["java.", "javax.", "jdk.", "sun.", "com.sun."];

impl OriginRules {
    pub fn from_info(info: &ModInfo) -> Self {
        Self { mod_ids: info.mod_id.iter().cloned().collect(), packages: info.packages.clone(), mods: Arc::default() }
    }

    /// Те же правила, но с модами запуска.
    pub fn with_mods(&self, mods: Arc<ModMap>) -> Self {
        Self { mods, ..self.clone() }
    }

    /// Происхождение класса по полному имени (`com.example.Foo$Bar`).
    pub fn class(&self, fqcn: &str) -> Origin {
        let starts = |list: &[&str]| list.iter().any(|p| fqcn.starts_with(p));
        if self.packages.iter().any(|p| fqcn.starts_with(p.as_str()) && fqcn[p.len()..].starts_with(['.', '$'])) {
            Origin::OwnMod
        } else if starts(MINECRAFT) {
            Origin::Minecraft
        } else if starts(NEOFORGE) {
            Origin::NeoForge
        } else if starts(FABRIC) {
            Origin::Fabric
        } else if starts(FORGE) {
            Origin::Forge
        } else if starts(JAVA) {
            Origin::Java
        } else {
            // Пакет класса: всё до последней точки (вложенные классы — через `$`).
            let package = fqcn.rsplit_once('.').map(|(p, _)| p).unwrap_or("");
            self.mods.packages.get(package).cloned().unwrap_or(Origin::Library)
        }
    }

    /// Происхождение по модулю из стек-трейса NeoForge: `TRANSFORMER/minecraft@1.21.1`,
    /// `TRANSFORMER/mymod@1.0.0`, `java.base`. Если модуль ничего не говорит — `None`.
    pub fn module(&self, module: &str) -> Option<Origin> {
        let last = module.rsplit('/').next().unwrap_or(module);
        let name = last.split('@').next().unwrap_or(last);
        if name.is_empty() {
            return None;
        }
        if self.mod_ids.iter().any(|id| id == name) {
            return Some(Origin::OwnMod);
        }
        if name.starts_with("java.") || name.starts_with("jdk.") {
            return Some(Origin::Java);
        }
        if let Some(o) = self.mods.modules.get(name) {
            return Some(o.clone());
        }
        // У модов в id точек не бывает, а у модулей библиотек они есть (`com.google.gson`,
        // `cpw.mods.modlauncher`): чьи они — решают правила по пакету класса.
        if last.contains('@') && !name.contains('.') {
            return Some(match name {
                "minecraft" => Origin::Minecraft,
                "neoforge" | "fml_loader" | "fml" => Origin::NeoForge,
                "forge" => Origin::Forge,
                "fabricloader" | "fabric_api" => Origin::Fabric,
                n => Origin::OtherMod(Some(n.to_owned())),
            });
        }
        None
    }

    /// Происхождение ресурса по пространству имён (`mymod:item/ruby`). Пространство, которое
    /// не id мода, принадлежит моду запуска, который единственный кладёт его в свои ресурсы.
    pub fn namespace(&self, ns: &str) -> Origin {
        match ns {
            n if self.mod_ids.iter().any(|id| id == n) => Origin::OwnMod,
            "minecraft" => Origin::Minecraft,
            "neoforge" | "c" => Origin::NeoForge,
            "fabric" => Origin::Fabric,
            "forge" => Origin::Forge,
            n => self.mods.namespaces.get(n).cloned().unwrap_or_else(|| Origin::OtherMod(Some(n.to_owned()))),
        }
    }

    /// Чья конфигурация миксинов (`railways-common.mixins.json`): из модов запуска, а если
    /// её там нет — по началу имени (`testmod.mixins.json` → мод `testmod`).
    pub fn mixin_config(&self, config: &str) -> Option<Origin> {
        if let Some(o) = self.mods.mixins.get(config) {
            return Some(o.clone());
        }
        let stem = config.split(['.', '-']).next().unwrap_or(config);
        if self.mod_ids.iter().any(|id| id == stem) {
            return Some(Origin::OwnMod);
        }
        self.mods.modules.get(stem).cloned()
    }

    /// Полное имя логгера из строки лога. В консоли NeoForge логгеры Minecraft записаны как
    /// `minecraft/Util`, остальные сокращены: `ne.ne.fm.ModLoader`, `co.ho.te.TestMod`.
    /// Сокращённое имя разворачивается по известным пакетам; неизвестное остаётся как есть.
    pub fn expand_logger(&self, logger: &str, marker: Option<&str>) -> String {
        let marker = marker.filter(|m| !m.is_empty());
        match (logger, marker) {
            ("minecraft", Some(m)) => return format!("net.minecraft.{m}"),
            ("mojang", Some(m)) => return format!("com.mojang.{m}"),
            _ => {}
        }
        let Some((package, class)) = logger.rsplit_once('.') else { return logger.to_owned() };
        // Сокращённое имя: все части пакета — не длиннее двух букв.
        if !package.split('.').all(|s| s.len() <= 2) {
            return logger.to_owned();
        }
        if let Some(full) = self.mods.abbreviated.get(package) {
            return format!("{full}.{class}");
        }
        let candidates = self.packages.iter().map(String::as_str).chain(KNOWN_PACKAGES.iter().copied());
        // Самый длинный известный пакет, сокращение которого — начало пакета логгера.
        let best = candidates
            .filter_map(|p| {
                let abbr = abbreviate_package(p);
                let rest = if package == abbr { Some("") } else { package.strip_prefix(abbr.as_str()).and_then(|r| r.strip_prefix('.')) };
                rest.map(|rest| (p, rest))
            })
            .max_by_key(|(p, _)| p.len());
        match best {
            Some((p, "")) => format!("{p}.{class}"),
            Some((p, rest)) => format!("{p}.{rest}.{class}"),
            None => logger.to_owned(),
        }
    }
}

/// Происхождение загрузчика как `Origin`.
pub fn loader_origin(loader: Loader) -> Origin {
    match loader {
        Loader::NeoForge => Origin::NeoForge,
        Loader::Fabric | Loader::Quilt => Origin::Fabric,
        Loader::Forge => Origin::Forge,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> OriginRules {
        OriginRules { mod_ids: vec!["testmod".into()], packages: vec!["com.holo.testmod".into()], mods: Arc::default() }
    }

    #[test]
    fn classes() {
        let r = rules();
        assert_eq!(r.class("com.holo.testmod.TestMod"), Origin::OwnMod);
        assert_eq!(r.class("com.holo.testmod.item.Ruby$Inner"), Origin::OwnMod);
        assert_eq!(r.class("com.holo.testmodextra.Other"), Origin::Library, "только целый сегмент пакета");
        assert_eq!(r.class("net.minecraft.client.Minecraft"), Origin::Minecraft);
        assert_eq!(r.class("com.mojang.blaze3d.systems.RenderSystem"), Origin::Minecraft);
        assert_eq!(r.class("net.neoforged.fml.ModLoader"), Origin::NeoForge);
        assert_eq!(r.class("net.fabricmc.loader.impl.FabricLoaderImpl"), Origin::Fabric);
        assert_eq!(r.class("java.lang.Thread"), Origin::Java);
        assert_eq!(r.class("org.spongepowered.asm.mixin.Mixin"), Origin::Library);
    }

    #[test]
    fn neoforge_modules() {
        let r = rules();
        assert_eq!(r.module("TRANSFORMER/minecraft@1.21.1"), Some(Origin::Minecraft));
        assert_eq!(r.module("TRANSFORMER/testmod@1.0.0"), Some(Origin::OwnMod));
        assert_eq!(r.module("TRANSFORMER/create@6.0.4"), Some(Origin::OtherMod(Some("create".into()))));
        assert_eq!(r.module("TRANSFORMER/neoforge@21.1.252"), Some(Origin::NeoForge));
        assert_eq!(r.module("java.base"), Some(Origin::Java));
        assert_eq!(r.module("MC-BOOTSTRAP"), None);
        assert_eq!(r.module("MC-BOOTSTRAP/com.google.gson@2.10.1"), None, "модуль библиотеки — не мод: решает пакет класса");
    }

    #[test]
    fn mods_of_the_run_own_their_packages_and_modules() {
        let mut map = ModMap::default();
        map.packages.insert("com.simibubi.create.content".into(), Origin::OtherMod(Some("create".into())));
        map.modules.insert("registrate".into(), Origin::Library);
        let r = rules().with_mods(Arc::new(map));
        assert_eq!(r.class("com.simibubi.create.content.Foo$Bar"), Origin::OtherMod(Some("create".into())));
        assert_eq!(r.class("com.simibubi.create.Other"), Origin::Library, "пакет должен совпасть целиком");
        assert_eq!(r.class("net.minecraft.world.item.Item"), Origin::Minecraft);
        assert_eq!(r.module("TRANSFORMER/registrate@1.3"), Some(Origin::Library));
        assert_eq!(r.module("TRANSFORMER/testmod@1.0.0"), Some(Origin::OwnMod));
    }

    #[test]
    fn console_loggers_expand() {
        let mut map = ModMap::default();
        map.abbreviated.insert("co.si.cr.co".into(), "com.simibubi.create.content".into());
        map.mixins.insert("railways-common.mixins.json".into(), Origin::OtherMod(Some("railways".into())));
        map.namespaces.insert("byg".into(), Origin::OtherMod(Some("railways".into())));
        let r = rules().with_mods(Arc::new(map));
        assert_eq!(r.expand_logger("minecraft", Some("Util")), "net.minecraft.Util");
        assert_eq!(r.expand_logger("mojang", Some("YggdrasilAuth")), "com.mojang.YggdrasilAuth");
        assert_eq!(r.expand_logger("ne.ne.fm.ModLoader", Some("LOADING")), "net.neoforged.fml.ModLoader");
        assert_eq!(r.expand_logger("co.ho.te.TestMod", None), "com.holo.testmod.TestMod");
        assert_eq!(r.expand_logger("co.ho.te.it.Ruby", None), "com.holo.testmod.it.Ruby", "неизвестный хвост остаётся сокращённым");
        assert_eq!(r.expand_logger("co.si.cr.co.Foo", None), "com.simibubi.create.content.Foo");
        assert_eq!(r.expand_logger("net.minecraft.Util", None), "net.minecraft.Util", "полное имя не трогаем");
        assert_eq!(r.expand_logger("mixin", None), "mixin");
        assert_eq!(r.class(&r.expand_logger("ne.ne.fm.ModLoader", None)), Origin::NeoForge);
        assert_eq!(r.mixin_config("railways-common.mixins.json"), Some(Origin::OtherMod(Some("railways".into()))));
        assert_eq!(r.mixin_config("testmod.mixins.json"), Some(Origin::OwnMod));
        assert_eq!(r.namespace("byg"), Origin::OtherMod(Some("railways".into())));
        assert_eq!(abbreviate_package("com.simibubi.create"), "co.si.cr");
    }

    #[test]
    fn namespaces() {
        let r = rules();
        assert_eq!(r.namespace("testmod"), Origin::OwnMod);
        assert_eq!(r.namespace("minecraft"), Origin::Minecraft);
        assert_eq!(r.namespace("create"), Origin::OtherMod(Some("create".into())));
    }
}
