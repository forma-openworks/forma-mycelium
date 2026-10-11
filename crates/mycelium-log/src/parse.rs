//! Построчный разбор вывода запуска: лог игры (форматы NeoForge, Fabric и ванилы),
//! стек-трейсы с цепочкой причин, краш-репорты, вывод Gradle и ошибки компиляторов.
//!
//! Строки приходят по одной, как из процесса. Запись считается законченной, когда
//! начинается следующая (стек-трейс идёт строками после заголовка), или по [`Parser::finish`].

use std::sync::LazyLock;

use mycelium_project::OriginRules;
use regex::Regex;

use crate::classify;
use crate::model::{Entry, Frame, Level, Source, SourceLocation, Throwable};

/// `[время] [поток/УРОВЕНЬ] [логгер/маркер]: текст` (NeoForge),
/// `[время] [поток/УРОВЕНЬ] (логгер) текст` (Fabric), `[время] [поток/УРОВЕНЬ]: текст` (ваниль).
static HEADER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^\[(?P<time>[^\]]{3,40})\] \[(?P<thread>[^\]]*?)/(?P<level>TRACE|DEBUG|INFO|WARN|ERROR|FATAL)\](?: \[(?P<logger>[^\]/]*)(?:/(?P<marker>[^\]]*))?\])?(?: \((?P<logger2>[^)]*)\))?:? ?(?P<msg>.*)$",
    )
    .expect("регулярное выражение заголовка")
});
static FRAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s+at (?P<body>[^\s(]+)\((?P<src>[^)]*)\)(?:\s*~?\[(?P<jar>[^\]]*)\])?").expect("регулярное выражение кадра")
});
static CAUSED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*Caused by: (?P<type>[\w$.]+)(?:: (?P<msg>.*))?$").expect("регулярное выражение причины"));
static SUPPRESSED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s+Suppressed: ").expect("регулярное выражение"));
static MORE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s+\.\.\. (?P<n>\d+) more").expect("регулярное выражение"));
static EXCEPTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:Exception in thread .+? )?(?P<type>(?:[a-zA-Z_$][\w$]*\.)+[A-Z][\w$]*(?:Exception|Error|Throwable|Failure)[\w$]*)(?:: (?P<msg>.*))?$")
        .expect("регулярное выражение исключения")
});
static JAVAC: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?P<file>.+\.(?:java|kt|kts)):(?P<line>\d+): (?P<kind>error|warning): (?P<msg>.*)$").expect("регулярное выражение javac")
});
static KOTLINC: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?P<kind>e|w): (?:file:///?)?(?P<file>.+\.kts?):(?P<line>\d+):(?P<col>\d+) (?P<msg>.*)$").expect("регулярное выражение kotlinc")
});
static COUNT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+ (?:errors?|warnings?)$").expect("регулярное выражение"));
static TASK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^> Task (?P<task>:\S+)(?: (?P<state>.+))?$").expect("регулярное выражение задачи"));

/// Служебные строки Gradle, которые не продолжают предыдущую запись.
const GRADLE_PREFIXES: &[&str] = &[
    "> Configure project",
    "Starting a Gradle Daemon",
    "Deprecated Gradle features",
    "You can use '--warning-mode",
    "For more on this, please refer",
    "Reusing configuration cache",
    "Configuration cache entry",
    "Calculating task graph",
    "[Incubating]",
    "To honour the JVM settings",
    "Daemon will be stopped",
];

/// Сколько строк краш-репорта храним (остальное — сведения о системе).
const CRASH_DETAIL_LIMIT: usize = 400;

#[derive(PartialEq)]
enum Mode {
    Normal,
    /// Внутри «Suppressed:» — эти кадры не относятся к основной цепочке.
    Suppressed,
    /// Блок «FAILURE: Build failed» у Gradle.
    GradleFailure,
    /// Краш-репорт: до «A detailed walkthrough» — исключение, дальше сведения.
    Crash { walkthrough: bool },
}

pub struct Parser {
    rules: OriginRules,
    current: Option<Entry>,
    mode: Mode,
    seq: u64,
}

impl Parser {
    pub fn new(rules: OriginRules) -> Self {
        Self { rules, current: None, mode: Mode::Normal, seq: 0 }
    }

    /// Правила происхождения изменились (например, проект перечитан).
    pub fn set_rules(&mut self, rules: OriginRules) {
        self.rules = rules;
    }

    /// Незаконченная запись (её ещё могут дополнить следующие строки).
    pub fn pending(&self) -> Option<&Entry> {
        self.current.as_ref()
    }

    /// Разбирает строку. Возвращает записи, которые этой строкой завершились.
    pub fn push(&mut self, raw: &str) -> Vec<Entry> {
        let line = raw.trim_end_matches(['\r', '\n']);
        let mut done = Vec::new();

        // Краш-репорт и блок ошибки Gradle собирают всё подряд, пока не начнётся новая запись.
        if line.starts_with("---- Minecraft Crash Report ----") {
            self.finish_into(&mut done);
            let mut e = self.entry(Level::Fatal, Source::Game, "Minecraft Crash Report");
            e.crash = true;
            self.current = Some(e);
            self.mode = Mode::Crash { walkthrough: false };
            return done;
        }

        if let Some(c) = HEADER.captures(line) {
            self.finish_into(&mut done);
            let level = Level::parse(&c["level"]).unwrap_or(Level::Info);
            let mut e = self.entry(level, Source::Game, &c["msg"]);
            e.time = Some(c["time"].to_owned());
            e.thread = Some(c["thread"].to_owned());
            // В консоли NeoForge логгеры сокращены (`minecraft/Util`, `ne.ne.fm.ModLoader`) —
            // разворачиваем, чтобы классификация видела то же, что в latest.log.
            let marker = c.name("marker").map(|m| m.as_str());
            e.logger = c.name("logger").or_else(|| c.name("logger2")).map(|m| self.rules.expand_logger(m.as_str(), marker)).filter(|s| !s.is_empty());
            if e.message.contains("#@!@# Game crashed!") {
                e.crash = true;
            }
            self.current = Some(e);
            return done;
        }

        if let Mode::Crash { walkthrough } = self.mode {
            self.crash_line(line, walkthrough);
            return done;
        }

        if let Some(c) = FRAME.captures(line) {
            if self.mode == Mode::Suppressed {
                return done;
            }
            let frame = self.frame(&c["body"], &c["src"], c.name("jar").map(|m| m.as_str()));
            if self.current.is_none() {
                let e = self.entry(Level::Error, Source::Process, "Stack trace");
                self.current = Some(e);
            }
            let cur = self.current.as_mut().expect("запись создана выше");
            cur.throwable.get_or_insert_with(|| Throwable::new("java.lang.Throwable", None)).last_mut().frames.push(frame);
            return done;
        }
        if let Some(c) = CAUSED.captures(line) {
            self.mode = Mode::Normal;
            if let Some(t) = self.current.as_mut().and_then(|e| e.throwable.as_mut()) {
                t.last_mut().cause = Some(Box::new(Throwable::new(&c["type"], c.name("msg").map(|m| m.as_str().to_owned()))));
                return done;
            }
        }
        if SUPPRESSED.is_match(line) && self.current.as_ref().is_some_and(|e| e.throwable.is_some()) {
            self.mode = Mode::Suppressed;
            return done;
        }
        if let Some(c) = MORE.captures(line) {
            if self.mode != Mode::Suppressed {
                if let Some(t) = self.current.as_mut().and_then(|e| e.throwable.as_mut()) {
                    t.last_mut().omitted = c["n"].parse().unwrap_or(0);
                }
            }
            return done;
        }

        if self.mode == Mode::GradleFailure {
            if line.starts_with("BUILD FAILED") || line.starts_with("* Get more help") {
                self.mode = Mode::Normal;
                self.finish_into(&mut done);
                if line.starts_with("BUILD FAILED") {
                    self.current = Some(self.entry(Level::Error, Source::Gradle, line));
                }
            } else if let Some(cur) = &mut self.current {
                if !line.trim().is_empty() {
                    cur.details.push(line.to_owned());
                }
            }
            return done;
        }

        if let Some(c) = EXCEPTION.captures(line) {
            let msg = c.name("msg").map(|m| m.as_str().to_owned());
            // Исключение сразу после строки лога — это его стек-трейс.
            if let Some(cur) = self.current.as_mut().filter(|e| e.throwable.is_none() && e.source == Source::Game) {
                cur.throwable = Some(Throwable::new(&c["type"], msg));
                self.mode = Mode::Normal;
                return done;
            }
            self.finish_into(&mut done);
            let mut e = self.entry(Level::Error, Source::Process, line);
            e.throwable = Some(Throwable::new(&c["type"], msg));
            self.current = Some(e);
            self.mode = Mode::Normal;
            return done;
        }

        if let Some(c) = JAVAC.captures(line) {
            self.finish_into(&mut done);
            let level = if &c["kind"] == "error" { Level::Error } else { Level::Warn };
            let mut e = self.entry(level, Source::Compiler, &c["msg"]);
            e.location = Some(SourceLocation { file: c["file"].to_owned(), line: c["line"].parse().unwrap_or(0), column: None });
            self.current = Some(e);
            return done;
        }
        if let Some(c) = KOTLINC.captures(line) {
            self.finish_into(&mut done);
            let level = if &c["kind"] == "e" { Level::Error } else { Level::Warn };
            let mut e = self.entry(level, Source::Compiler, &c["msg"]);
            e.location = Some(SourceLocation {
                file: c["file"].to_owned(),
                line: c["line"].parse().unwrap_or(0),
                column: c["col"].parse().ok(),
            });
            self.current = Some(e);
            return done;
        }
        if let Some(c) = TASK.captures(line) {
            self.finish_into(&mut done);
            let failed = c.name("state").is_some_and(|s| s.as_str() == "FAILED");
            let e = self.entry(if failed { Level::Error } else { Level::Info }, Source::Gradle, line);
            self.current = Some(e);
            return done;
        }
        if let Some(rest) = line.strip_prefix("FAILURE: ") {
            self.finish_into(&mut done);
            self.current = Some(self.entry(Level::Error, Source::Gradle, rest));
            self.mode = Mode::GradleFailure;
            return done;
        }
        if line.starts_with("BUILD SUCCESSFUL")
            || line.starts_with("BUILD FAILED")
            || GRADLE_PREFIXES.iter().any(|p| line.starts_with(p))
            || COUNT.is_match(line)
        {
            self.finish_into(&mut done);
            let level = if line.starts_with("BUILD FAILED") { Level::Error } else { Level::Info };
            self.current = Some(self.entry(level, Source::Gradle, line));
            return done;
        }

        if line.trim().is_empty() {
            return done;
        }
        // Продолжение многострочного сообщения игры или компилятора (код и «^» под ошибкой).
        if let Some(cur) = self.current.as_mut().filter(|e| matches!(e.source, Source::Game | Source::Compiler) && e.throwable.is_none()) {
            cur.details.push(line.to_owned());
            return done;
        }
        self.finish_into(&mut done);
        self.current = Some(self.entry(Level::Info, Source::Process, line));
        done
    }

    /// Завершает незаконченную запись (процесс завершился или долго молчит).
    pub fn finish(&mut self) -> Vec<Entry> {
        let mut done = Vec::new();
        self.finish_into(&mut done);
        done
    }

    fn finish_into(&mut self, done: &mut Vec<Entry>) {
        self.mode = Mode::Normal;
        if let Some(mut e) = self.current.take() {
            classify::classify(&mut e, &self.rules);
            done.push(e);
        }
    }

    fn entry(&mut self, level: Level, source: Source, message: &str) -> Entry {
        self.seq += 1;
        Entry::new(self.seq, level, source, message)
    }

    fn crash_line(&mut self, line: &str, walkthrough: bool) {
        let Some(cur) = self.current.as_mut() else { return };
        if line.starts_with("A detailed walkthrough of the error") {
            self.mode = Mode::Crash { walkthrough: true };
            return;
        }
        if !walkthrough {
            if let Some(desc) = line.strip_prefix("Description: ") {
                cur.message = desc.to_owned();
                return;
            }
            if cur.throwable.is_none() {
                if let Some(c) = EXCEPTION.captures(line) {
                    cur.throwable = Some(Throwable::new(&c["type"], c.name("msg").map(|m| m.as_str().to_owned())));
                    return;
                }
            }
            if let Some(c) = FRAME.captures(line) {
                let frame = Self::make_frame(&self.rules, &c["body"], &c["src"], c.name("jar").map(|m| m.as_str()));
                if let Some(t) = cur.throwable.as_mut() {
                    t.last_mut().frames.push(frame);
                }
                return;
            }
            if let Some(c) = CAUSED.captures(line) {
                if let Some(t) = cur.throwable.as_mut() {
                    t.last_mut().cause = Some(Box::new(Throwable::new(&c["type"], c.name("msg").map(|m| m.as_str().to_owned()))));
                }
                return;
            }
            if let Some(c) = MORE.captures(line) {
                if let Some(t) = cur.throwable.as_mut() {
                    t.last_mut().omitted = c["n"].parse().unwrap_or(0);
                }
                return;
            }
        }
        if !line.trim().is_empty() && cur.details.len() < CRASH_DETAIL_LIMIT {
            cur.details.push(line.to_owned());
        }
    }

    fn frame(&self, body: &str, src: &str, jar: Option<&str>) -> Frame {
        Self::make_frame(&self.rules, body, src, jar)
    }

    fn make_frame(rules: &OriginRules, body: &str, src: &str, jar: Option<&str>) -> Frame {
        // `МОДУЛЬ/.../пакет.Класс.метод`: модуль — всё до последнего «/».
        let (module, qualified) = match body.rfind('/') {
            Some(i) => (Some(body[..i].to_owned()), &body[i + 1..]),
            None => (None, body),
        };
        let (class, method) = match qualified.rfind('.') {
            Some(i) => (qualified[..i].to_owned(), qualified[i + 1..].to_owned()),
            None => (qualified.to_owned(), String::new()),
        };
        let (file, line) = match src.rsplit_once(':') {
            Some((f, l)) if l.chars().all(|c| c.is_ascii_digit()) && !l.is_empty() => (Some(f.to_owned()), l.parse().ok()),
            _ if src.ends_with(".java") || src.ends_with(".kt") => (Some(src.to_owned()), None),
            _ => (None, None),
        };
        let origin = module.as_deref().and_then(|m| rules.module(m)).unwrap_or_else(|| rules.class(&class));
        Frame { module, class, method, file, line, jar: jar.map(str::to_owned).filter(|j| j != "?:?" && !j.is_empty()), origin }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Known, System};
    use mycelium_project::Origin;

    fn rules() -> OriginRules {
        OriginRules { mod_ids: vec!["testmod".into()], packages: vec!["com.holo.testmod".into()], ..OriginRules::default() }
    }

    fn parse(text: &str) -> Vec<Entry> {
        let mut p = Parser::new(rules());
        let mut out = Vec::new();
        for line in text.lines() {
            out.extend(p.push(line));
        }
        out.extend(p.finish());
        out
    }

    #[test]
    fn neoforge_line_with_logger_and_marker() {
        let e = parse("[11Oct2026 00:12:34.567] [main/INFO] [net.neoforged.fml.loading.FMLLoader/CORE]: Loading mods");
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].level, Level::Info);
        assert_eq!(e[0].thread.as_deref(), Some("main"));
        assert_eq!(e[0].logger.as_deref(), Some("net.neoforged.fml.loading.FMLLoader"));
        assert_eq!(e[0].message, "Loading mods");
        assert_eq!(e[0].system, System::ModLoading);
    }

    #[test]
    fn neoforge_console_loggers_are_expanded() {
        let e = parse(
            "[13:13:57] [Worker-Main-5/ERROR] [minecraft/Util]: Invalid path in pack: railways:models/block/X.bbmodel, ignoring\n[13:13:58] [main/INFO] [ne.ne.fm.ModLoader/LOADING]: Loading\n[13:13:59] [modloading-worker-0/ERROR] [co.ho.te.TestMod/]: Не удалось описать рубин",
        );
        assert_eq!(e[0].logger.as_deref(), Some("net.minecraft.Util"));
        assert_eq!(e[0].via, Some(Origin::Minecraft));
        assert_eq!(e[1].logger.as_deref(), Some("net.neoforged.fml.ModLoader"));
        assert_eq!(e[1].system, System::ModLoading);
        assert_eq!(e[2].logger.as_deref(), Some("com.holo.testmod.TestMod"));
        assert_eq!(e[2].origin, Origin::OwnMod);
    }

    #[test]
    fn fabric_and_vanilla_lines() {
        let e = parse("[00:12:34] [main/INFO] (FabricLoader/GameProvider) Loading Minecraft 1.21.1\n[00:12:35] [Render thread/WARN]: Something odd");
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].logger.as_deref(), Some("FabricLoader/GameProvider"));
        assert_eq!(e[0].message, "Loading Minecraft 1.21.1");
        assert_eq!(e[1].thread.as_deref(), Some("Render thread"));
        assert_eq!(e[1].level, Level::Warn);
        assert_eq!(e[1].message, "Something odd");
    }

    #[test]
    fn stack_trace_with_causes_and_modules() {
        let log = "\
[11Oct2026 00:12:40.001] [Render thread/ERROR] [com.holo.testmod.TestMod/]: Failed to set up ruby
java.lang.IllegalStateException: Ruby setup failed
\tat TRANSFORMER/testmod@1.0.0/com.holo.testmod.TestMod.setup(TestMod.java:42) ~[%23188!/:?]
\tat TRANSFORMER/neoforge@21.1.252/net.neoforged.fml.ModContainer.lambda$1(ModContainer.java:120) ~[neoforge-21.1.252.jar:?]
\tat java.base/java.lang.Thread.run(Thread.java:1583) [?:?]
Caused by: java.lang.NullPointerException: Cannot invoke \"Object.toString()\" because \"value\" is null
\tat TRANSFORMER/testmod@1.0.0/com.holo.testmod.item.Ruby.describe(Ruby.java:17) ~[%23188!/:?]
\t... 3 more
[11Oct2026 00:12:41.000] [Render thread/INFO] [net.minecraft.client.Minecraft/]: Next";
        let e = parse(log);
        assert_eq!(e.len(), 2);
        let t = e[0].throwable.as_ref().unwrap();
        assert_eq!(t.type_name, "java.lang.IllegalStateException");
        assert_eq!(t.message.as_deref(), Some("Ruby setup failed"));
        assert_eq!(t.frames.len(), 3);
        assert_eq!(t.frames[0].origin, Origin::OwnMod);
        assert_eq!(t.frames[0].line, Some(42));
        assert_eq!(t.frames[0].short(), "TestMod.setup:42");
        assert_eq!(t.frames[1].origin, Origin::NeoForge);
        assert_eq!(t.frames[1].jar.as_deref(), Some("neoforge-21.1.252.jar:?"));
        assert_eq!(t.frames[2].origin, Origin::Java);
        let root = t.root_cause();
        assert_eq!(root.short_type(), "NullPointerException");
        assert_eq!(root.omitted, 3);
        assert_eq!(e[0].origin, Origin::OwnMod, "виноват свой код");
        assert_eq!(e[0].known, Some(Known::NullPointer));
    }

    #[test]
    fn missing_texture_multiline_warning() {
        let log = "\
[11Oct2026 00:13:00.000] [Worker-Main-3/WARN] [net.minecraft.client.resources.model.ModelManager/]: Missing textures in model testmod:item/ruby#inventory:
    minecraft:textures/atlas/blocks.png:testmod:item/ruby
[11Oct2026 00:13:00.100] [Worker-Main-3/WARN] [net.minecraft.client.resources.model.ModelManager/]: Unable to load model: 'minecraft:item/x' referenced from: x";
        let e = parse(log);
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].details, vec!["    minecraft:textures/atlas/blocks.png:testmod:item/ruby".to_owned()]);
        assert_eq!(e[0].known, Some(Known::MissingTexture));
        assert_eq!(e[0].system, System::Resources);
        assert_eq!(e[0].origin, Origin::OwnMod, "пространство имён своего мода");
        assert_eq!(e[1].known, Some(Known::MissingModel));
        assert_eq!(e[1].origin, Origin::Minecraft);
    }

    #[test]
    fn crash_report() {
        let log = "\
---- Minecraft Crash Report ----
// Who set us up the TNT?

Time: 2026-10-11 00:20:00
Description: Mod loading failures have occurred; consult the issue messages for more details

java.lang.RuntimeException: Boom
\tat TRANSFORMER/testmod@1.0.0/com.holo.testmod.TestMod.<init>(TestMod.java:10) ~[%23188!/:?]

A detailed walkthrough of the error, its code path and all known details is as follows:
---------------------------------------------------------------------------------------
-- System Details --
\tMinecraft Version: 1.21.1
[11Oct2026 00:20:01.000] [main/FATAL] [net.minecraft.client.Minecraft/]: #@!@# Game crashed! Crash report saved to: #@!@# C:\\crash.txt";
        let e = parse(log);
        assert_eq!(e.len(), 2);
        assert!(e[0].crash);
        assert_eq!(e[0].level, Level::Fatal);
        assert_eq!(e[0].message, "Mod loading failures have occurred; consult the issue messages for more details");
        let t = e[0].throwable.as_ref().unwrap();
        assert_eq!(t.short_type(), "RuntimeException");
        assert_eq!(t.frames.len(), 1, "кадры из сведений о системе не попадают в трейс");
        assert!(e[0].details.iter().any(|d| d.contains("Minecraft Version")));
        assert!(e[1].crash);
    }

    #[test]
    fn javac_and_kotlin_errors() {
        let log = "\
> Task :compileJava FAILED
C:\\mods\\testmod\\src\\main\\java\\com\\holo\\testmod\\TestMod.java:12: error: cannot find symbol
        Rubby.init();
        ^
  symbol:   variable Rubby
e: file:///C:/mods/testmod/src/main/kotlin/Foo.kt:5:9 Unresolved reference 'x'.
1 error

FAILURE: Build failed with an exception.

* What went wrong:
Execution failed for task ':compileJava'.
> Compilation failed; see the compiler error output for details.

* Try:
> Run with --info option to get more log output.

BUILD FAILED in 3s";
        let e = parse(log);
        let task = &e[0];
        assert_eq!(task.level, Level::Error, "задача упала");
        let javac = &e[1];
        assert_eq!(javac.source, Source::Compiler);
        assert_eq!(javac.message, "cannot find symbol");
        let loc = javac.location.as_ref().unwrap();
        assert_eq!(loc.line, 12);
        assert!(loc.file.ends_with("TestMod.java"));
        assert_eq!(javac.details.len(), 3, "код, указатель и symbol");
        assert_eq!(javac.known, Some(Known::CompileError));
        let kt = &e[2];
        assert_eq!(kt.location.as_ref().unwrap().column, Some(9));
        let failure = e.iter().find(|x| x.message.starts_with("Build failed")).unwrap();
        assert!(failure.details.iter().any(|d| d.contains("Execution failed for task ':compileJava'")));
        assert_eq!(failure.known, Some(Known::BuildFailed));
        assert!(e.last().unwrap().message.starts_with("BUILD FAILED"));
    }

    #[test]
    fn standalone_exception_and_plain_lines() {
        let log = "\
Starting a Gradle Daemon (subsequent builds will be faster)
Exception in thread \"main\" java.lang.NoClassDefFoundError: net/minecraft/client/Minecraft
\tat com.holo.testmod.Main.main(Main.java:3)
some plain output";
        let e = parse(log);
        assert_eq!(e.len(), 3);
        assert_eq!(e[0].source, Source::Gradle);
        assert_eq!(e[1].throwable.as_ref().unwrap().short_type(), "NoClassDefFoundError");
        assert_eq!(e[1].known, Some(Known::ClassMissing));
        assert_eq!(e[2].message, "some plain output");
    }

    #[test]
    fn suppressed_frames_are_skipped() {
        let log = "\
[11Oct2026 00:12:40.001] [main/ERROR] [x/]: oops
java.io.IOException: closed
\tat a.b.C.d(C.java:1)
\tSuppressed: java.io.IOException: also
\t\tat x.y.Z.w(Z.java:2)
Caused by: java.io.EOFException
\tat e.f.G.h(G.java:3)";
        let e = parse(log);
        let t = e[0].throwable.as_ref().unwrap();
        assert_eq!(t.frames.len(), 1);
        assert_eq!(t.root_cause().frames.len(), 1);
        assert_eq!(t.root_cause().short_type(), "EOFException");
    }
}
