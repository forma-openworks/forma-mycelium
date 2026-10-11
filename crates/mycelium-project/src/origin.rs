//! Происхождение кода и ресурсов: мой мод, Minecraft, загрузчик, другой мод, библиотека, Java.
//! Одна и та же палитра происхождения используется везде — в дереве, консоли, исключениях
//! и (позже) на карте мода.

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

/// Что знаем о своём моде, чтобы отличать его код от чужого.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OriginRules {
    pub mod_ids: Vec<String>,
    pub packages: Vec<String>,
}

/// Пакеты Minecraft (включая библиотеки Mojang, которые идут с игрой).
const MINECRAFT: &[&str] = &["net.minecraft.", "com.mojang."];
const NEOFORGE: &[&str] = &["net.neoforged.", "cpw.mods."];
const FABRIC: &[&str] = &["net.fabricmc."];
const FORGE: &[&str] = &["net.minecraftforge."];
const JAVA: &[&str] = &["java.", "javax.", "jdk.", "sun.", "com.sun."];

impl OriginRules {
    pub fn from_info(info: &ModInfo) -> Self {
        Self { mod_ids: info.mod_id.iter().cloned().collect(), packages: info.packages.clone() }
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
            Origin::Library
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
        if last.contains('@') {
            return Some(match name {
                n if self.mod_ids.iter().any(|id| id == n) => Origin::OwnMod,
                "minecraft" => Origin::Minecraft,
                "neoforge" | "fml_loader" | "fml" => Origin::NeoForge,
                "forge" => Origin::Forge,
                "fabricloader" | "fabric_api" => Origin::Fabric,
                n => Origin::OtherMod(Some(n.to_owned())),
            });
        }
        if name.starts_with("java.") || name.starts_with("jdk.") {
            return Some(Origin::Java);
        }
        None
    }

    /// Происхождение ресурса по пространству имён (`mymod:item/ruby`).
    pub fn namespace(&self, ns: &str) -> Origin {
        match ns {
            n if self.mod_ids.iter().any(|id| id == n) => Origin::OwnMod,
            "minecraft" => Origin::Minecraft,
            "neoforge" | "c" => Origin::NeoForge,
            "fabric" => Origin::Fabric,
            "forge" => Origin::Forge,
            n => Origin::OtherMod(Some(n.to_owned())),
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
        OriginRules { mod_ids: vec!["testmod".into()], packages: vec!["com.holo.testmod".into()] }
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
    }

    #[test]
    fn namespaces() {
        let r = rules();
        assert_eq!(r.namespace("testmod"), Origin::OwnMod);
        assert_eq!(r.namespace("minecraft"), Origin::Minecraft);
        assert_eq!(r.namespace("create"), Origin::OtherMod(Some("create".into())));
    }
}
