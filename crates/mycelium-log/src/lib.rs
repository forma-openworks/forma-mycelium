//! Разбор и классификация вывода запуска мода: лог игры, Gradle и компиляторы.
//!
//! Каждая запись получает серьёзность, систему игры (ресурсы, реестры, миксины…),
//! происхождение (мой мод, Minecraft, загрузчик, другой мод) и, если узнаётся,
//! известную проблему с объяснением простыми словами.

pub mod classify;
pub mod collect;
pub mod model;
pub mod parse;

pub use collect::{Problem, ProblemStore};
pub use model::{Entry, Frame, Known, Level, Source, SourceLocation, System, Throwable};
pub use parse::Parser;
