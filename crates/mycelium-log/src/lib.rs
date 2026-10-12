//! Разбор и классификация вывода запуска мода: лог игры, Gradle и компиляторы.
//!
//! Каждая запись получает серьёзность, систему игры (ресурсы, реестры, миксины…),
//! происхождение (мой мод, Minecraft, загрузчик, другой мод) и, если узнаётся,
//! известную проблему с объяснением простыми словами.

pub mod classify;
pub mod collect;
pub mod model;
pub mod parse;
pub mod tokens;

pub use collect::{Problem, ProblemStore};
pub use model::{Entry, Field, Frame, Known, Level, Role, RunSide, Source, SourceLocation, System, ThreadKind, Throwable};
pub use tokens::Token;
pub use parse::Parser;
