//! Hyphae — движок знаний Forma Mycelium: понимает мод целиком, а не только его Java.
//!
//! Слои:
//! - **читалки** — jar, метаданные модов, вложенные jar ([`jar`], [`meta`]); дальше —
//!   файлы классов, конфигурации миксинов, AT/AW и ресурсы;
//! - **индекс** — что известно о запуске и проекте ([`mods`]: моды запуска);
//! - **анализаторы** — выводы из индекса: чей это код, ресурс или строка лога.
//!
//! Hyphae — обычная библиотека: интерфейса у неё нет, Forma Mycelium просто вызывает её.

pub mod jar;
pub mod meta;
pub mod mixin;
pub mod mods;

pub use mixin::{MixinConfig, MixinSide};
pub use mods::{Found, ModKind, RunInput, RunMod, RunMods};
