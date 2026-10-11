//! Записи лога: строка с уровнем и логгером, стек-трейс с цепочкой причин,
//! и то, что о записи узнала классификация.

use mycelium_project::Origin;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

impl Level {
    pub fn parse(s: &str) -> Option<Level> {
        Some(match s.trim().to_ascii_uppercase().as_str() {
            "TRACE" => Level::Trace,
            "DEBUG" => Level::Debug,
            "INFO" => Level::Info,
            "WARN" | "WARNING" => Level::Warn,
            "ERROR" | "SEVERE" => Level::Error,
            "FATAL" => Level::Fatal,
            _ => return None,
        })
    }

    /// Проблема: предупреждение и серьёзнее.
    pub fn is_problem(self) -> bool {
        self >= Level::Warn
    }
}

/// Откуда пришла строка.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    /// Лог игры (log4j).
    Game,
    /// Вывод Gradle.
    Gradle,
    /// Ошибки и предупреждения компилятора (javac, kotlinc).
    Compiler,
    /// Прочий вывод процесса.
    Process,
}

/// Система игры, к которой относится запись.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum System {
    ModLoading,
    Registries,
    Resources,
    Data,
    Mixin,
    Network,
    Rendering,
    Sound,
    World,
    Config,
    Build,
    Game,
}

impl System {
    pub const ALL: [System; 12] = [
        System::ModLoading,
        System::Registries,
        System::Resources,
        System::Data,
        System::Mixin,
        System::Network,
        System::Rendering,
        System::Sound,
        System::World,
        System::Config,
        System::Build,
        System::Game,
    ];

    /// Ключ для перевода (`system-<ключ>`).
    pub fn key(self) -> &'static str {
        match self {
            System::ModLoading => "mod-loading",
            System::Registries => "registries",
            System::Resources => "resources",
            System::Data => "data",
            System::Mixin => "mixin",
            System::Network => "network",
            System::Rendering => "rendering",
            System::Sound => "sound",
            System::World => "world",
            System::Config => "config",
            System::Build => "build",
            System::Game => "game",
        }
    }
}

/// Кадр стек-трейса.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// Модуль или загрузчик классов: `TRANSFORMER/minecraft@1.21.1`, `java.base`.
    pub module: Option<String>,
    pub class: String,
    pub method: String,
    /// Файл исходника из скобок: `Foo.java`.
    pub file: Option<String>,
    pub line: Option<u32>,
    /// Jar из расширенного стек-трейса log4j: `~[minecraft-1.21.1.jar:?]`.
    pub jar: Option<String>,
    pub origin: Origin,
}

impl Frame {
    /// Короткая запись: `Foo.bar:12`.
    pub fn short(&self) -> String {
        let simple = self.class.rsplit('.').next().unwrap_or(&self.class);
        match self.line {
            Some(l) => format!("{simple}.{}:{l}", self.method),
            None => format!("{simple}.{}", self.method),
        }
    }
}

/// Исключение с цепочкой причин.
#[derive(Clone, Debug, PartialEq)]
pub struct Throwable {
    pub type_name: String,
    pub message: Option<String>,
    pub frames: Vec<Frame>,
    /// «... 12 more»: сколько кадров совпадает с исключением выше и опущено.
    pub omitted: usize,
    pub cause: Option<Box<Throwable>>,
}

impl Throwable {
    pub fn new(type_name: impl Into<String>, message: Option<String>) -> Self {
        Self { type_name: type_name.into(), message, frames: Vec::new(), omitted: 0, cause: None }
    }

    /// Само исключение и все его причины по порядку.
    pub fn chain(&self) -> impl Iterator<Item = &Throwable> {
        std::iter::successors(Some(self), |t| t.cause.as_deref())
    }

    /// Самая глубокая причина — обычно она и объясняет, что случилось.
    pub fn root_cause(&self) -> &Throwable {
        self.chain().last().unwrap_or(self)
    }

    pub fn last_mut(&mut self) -> &mut Throwable {
        match self.cause {
            Some(ref mut c) => c.last_mut(),
            None => self,
        }
    }

    /// Имя типа без пакета: `NullPointerException`.
    pub fn short_type(&self) -> &str {
        self.type_name.rsplit('.').next().unwrap_or(&self.type_name)
    }
}

/// Место в исходниках (для ошибок компилятора).
#[derive(Clone, Debug, PartialEq)]
pub struct SourceLocation {
    pub file: String,
    pub line: u32,
    pub column: Option<u32>,
}

/// Известная проблема: для неё есть объяснение простыми словами.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Known {
    MissingTexture,
    MissingModel,
    BlockstateError,
    InvalidResourceLocation,
    ClientClassOnServer,
    MixinFailure,
    RegistryNotBound,
    ClassMissing,
    NullPointer,
    DataParseError,
    MissingDependency,
    DuplicateMod,
    ModConstruction,
    OutOfMemory,
    ConcurrentModification,
    CompileError,
    BuildFailed,
}

impl Known {
    pub const ALL: [Known; 17] = [
        Known::MissingTexture,
        Known::MissingModel,
        Known::BlockstateError,
        Known::InvalidResourceLocation,
        Known::ClientClassOnServer,
        Known::MixinFailure,
        Known::RegistryNotBound,
        Known::ClassMissing,
        Known::NullPointer,
        Known::DataParseError,
        Known::MissingDependency,
        Known::DuplicateMod,
        Known::ModConstruction,
        Known::OutOfMemory,
        Known::ConcurrentModification,
        Known::CompileError,
        Known::BuildFailed,
    ];

    /// Ключ для перевода (`known-<ключ>-title`, `known-<ключ>-explain`).
    pub fn key(self) -> &'static str {
        match self {
            Known::MissingTexture => "missing-texture",
            Known::MissingModel => "missing-model",
            Known::BlockstateError => "blockstate",
            Known::InvalidResourceLocation => "invalid-id",
            Known::ClientClassOnServer => "client-on-server",
            Known::MixinFailure => "mixin",
            Known::RegistryNotBound => "registry-unbound",
            Known::ClassMissing => "class-missing",
            Known::NullPointer => "null-pointer",
            Known::DataParseError => "data-parse",
            Known::MissingDependency => "missing-dependency",
            Known::DuplicateMod => "duplicate-mod",
            Known::ModConstruction => "mod-construction",
            Known::OutOfMemory => "out-of-memory",
            Known::ConcurrentModification => "concurrent-modification",
            Known::CompileError => "compile-error",
            Known::BuildFailed => "build-failed",
        }
    }

    /// Система игры, к которой относится проблема.
    pub fn system(self) -> System {
        match self {
            Known::MissingTexture | Known::MissingModel | Known::BlockstateError => System::Resources,
            Known::InvalidResourceLocation | Known::RegistryNotBound => System::Registries,
            Known::ClientClassOnServer | Known::MissingDependency | Known::DuplicateMod | Known::ModConstruction | Known::ClassMissing => {
                System::ModLoading
            }
            Known::MixinFailure => System::Mixin,
            Known::DataParseError => System::Data,
            Known::NullPointer | Known::OutOfMemory | Known::ConcurrentModification => System::Game,
            Known::CompileError | Known::BuildFailed => System::Build,
        }
    }
}

/// Запись лога.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// Порядковый номер записи в запуске.
    pub seq: u64,
    pub time: Option<String>,
    pub thread: Option<String>,
    pub level: Level,
    pub logger: Option<String>,
    pub source: Source,
    /// Первая строка сообщения.
    pub message: String,
    /// Продолжение многострочного сообщения (без стек-трейса).
    pub details: Vec<String>,
    pub throwable: Option<Throwable>,
    /// Краш-репорт: игра упала.
    pub crash: bool,
    pub location: Option<SourceLocation>,
    // Классификация.
    pub system: System,
    pub origin: Origin,
    pub known: Option<Known>,
    /// Безобидный шум самого Minecraft или загрузчика в среде разработки: такие
    /// предупреждения есть у любого мода, и исправлять в своём коде нечего.
    pub noise: bool,
}

impl Entry {
    pub fn new(seq: u64, level: Level, source: Source, message: impl Into<String>) -> Self {
        Self {
            seq,
            time: None,
            thread: None,
            level,
            logger: None,
            source,
            message: message.into(),
            details: Vec::new(),
            throwable: None,
            crash: false,
            location: None,
            system: System::Game,
            origin: Origin::Unknown,
            known: None,
            noise: false,
        }
    }

    /// Проблема ли это: предупреждение, ошибка, исключение или краш.
    pub fn is_problem(&self) -> bool {
        self.level.is_problem() || self.throwable.is_some() || self.crash
    }

    /// Весь текст записи одной строкой — для поиска.
    pub fn search_text(&self) -> String {
        let mut s = self.message.clone();
        for d in &self.details {
            s.push('\n');
            s.push_str(d);
        }
        if let Some(t) = &self.throwable {
            for c in t.chain() {
                s.push('\n');
                s.push_str(&c.type_name);
                if let Some(m) = &c.message {
                    s.push_str(": ");
                    s.push_str(m);
                }
            }
        }
        s
    }
}
