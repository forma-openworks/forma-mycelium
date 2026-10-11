//! Классификация записи: к какой системе игры относится, чья это проблема и через
//! кого она всплыла, и не известная ли это ошибка, которую можно объяснить простыми словами.

use std::sync::LazyLock;

use mycelium_project::{Origin, OriginRules};
use regex::Regex;

use crate::model::{Entry, Known, Source, System, Throwable};

/// Идентификатор ресурса `пространство:путь` (`testmod:item/ruby`).
static RESOURCE_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b([a-z][a-z0-9_.-]*):([a-z0-9_.-]+(?:/[a-z0-9_./-]*)?)").expect("регулярное выражение id ресурса"));
/// Неправильный id ресурса: после `пространство:` идут любые символы (`railways:Bad Name`),
/// но не `//` (это адрес в интернете).
static ANY_ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b([a-z][a-z0-9_.-]*):(?:[^\s:/'\x22]|/[^/\s])").expect("регулярное выражение неправильного id"));

pub fn classify(e: &mut Entry, rules: &OriginRules) {
    e.known = detect_known(e);
    // У общих проблем (NPE и т. п.) система «Игра» ничего не говорит — уточняем по тексту и потоку.
    e.system = match e.known.map(Known::system) {
        Some(system) if system != System::Game => system,
        _ => detect_system(e),
    };
    e.origin = detect_origin(e, rules);
    e.via = detect_via(e, rules).filter(|v| *v != e.origin);
    e.noise = e.known.is_none() && e.origin != Origin::OwnMod && is_noise(e);
}

/// Предупреждения, которые Minecraft и загрузчики пишут при каждом запуске в среде
/// разработки. К моду они отношения не имеют.
const NOISE: &[&[&str]] = &[
    // Brigadier: команды /teleport неоднозначны в самой игре.
    &["ambiguity between arguments"],
    // NeoForge в среде разработки: ассеты лежат не в jar.
    &["assets url", "unexpected schema"],
    // В звуках ванилы есть ссылки на файлы, которых нет.
    &["missing sound for event: minecraft:"],
    // Ванильные шейдеры объявляют лишние семплеры.
    &["could not find sampler named"],
    // Mixin в среде разработки без refmap.
    &["reference map", "could not be read"],
    // Вход без аккаунта Microsoft: Realms, профиль, телеметрия.
    &["realms"],
    &["failed to fetch user properties"],
    &["failed to verify authentication"],
    &["couldn't query profile"],
    &["failed to send telemetry"],
    &["narrator library"],
    // Первый запуск: конфиги создаются заново.
    &["configuration file", "is not correct. correcting"],
    // Мелкие текстуры уменьшают число уровней mip-карт — так и задумано.
    &["limits mip level"],
];

fn is_noise(e: &Entry) -> bool {
    let text = e.message.to_lowercase();
    NOISE.iter().any(|all| all.iter().all(|n| text.contains(n)))
}

/// Текст для поиска признаков: сообщение, продолжение и цепочка исключений, строчными буквами.
fn haystack(e: &Entry) -> String {
    e.search_text().to_lowercase()
}

fn detect_known(e: &Entry) -> Option<Known> {
    if e.source == Source::Compiler && e.level >= crate::model::Level::Error {
        return Some(Known::CompileError);
    }
    if e.source == Source::Gradle && haystack(e).contains("build failed") {
        return Some(Known::BuildFailed);
    }
    // Сначала самая глубокая причина: обёртки сверху («не удалось загрузить модель»)
    // говорят только, где всё сломалось, а причина внизу — что именно.
    if let Some(t) = &e.throwable {
        let chain: Vec<&Throwable> = t.chain().collect();
        for c in chain.iter().rev() {
            let text = format!("{}: {}", c.type_name, c.message.as_deref().unwrap_or("")).to_lowercase();
            if let Some(k) = known_in(&text, &[c.short_type()]) {
                return Some(k);
            }
        }
    }
    let types: Vec<&str> = e.throwable.iter().flat_map(|t| t.chain()).map(|t| t.short_type()).collect();
    known_in(&haystack(e), &types)
}

/// Известная проблема по тексту (строчными буквами) и коротким именам типов исключений.
fn known_in(text: &str, types: &[&str]) -> Option<Known> {
    let has = |needles: &[&str]| needles.iter().any(|n| text.contains(n));
    let has_type = |names: &[&str]| types.iter().any(|t| names.contains(t));

    // Порядок важен: более точные признаки — раньше.
    if has(&["material was not found in the library"]) {
        return Some(Known::ObjMaterial);
    }
    if has(&["could not find obj model", "could not read obj model", "obj material library", "obj loader requires a 'model' key"]) {
        return Some(Known::ObjFile);
    }
    if has(&["attempted to load class"]) && has(&["invalid dist"]) || has(&["in environment type server"]) {
        return Some(Known::ClientClassOnServer);
    }
    if has(&["mixin apply failed", "invalidinjectionexception", "critical injection failure", "mixintransformererror", "invalidmixinexception"]) {
        return Some(Known::MixinFailure);
    }
    if has(&["invalid path in pack"]) {
        return Some(Known::InvalidResourcePath);
    }
    if has(&["non [a-z0-9", "resourcelocationexception", "invalid resource location", "identifierexception"]) {
        return Some(Known::InvalidResourceLocation);
    }
    if has(&["missing texture", "unable to resolve texture", "unable to load texture", "texture reference", "missing sprite"]) {
        return Some(Known::MissingTexture);
    }
    if has(&["blockstate"]) && has(&["exception loading", "missing", "unable", "failed"]) {
        return Some(Known::BlockstateError);
    }
    if has(&["unable to load model", "missing model", "unable to resolve model", "no model for", "failed to load model"]) {
        return Some(Known::MissingModel);
    }
    if has(&["registry object not present", "unbound value", "trying to access unbound", "not bound"]) {
        return Some(Known::RegistryNotBound);
    }
    if has(&["couldn't parse data file", "parsing error loading recipe", "couldn't load tag", "failed to load loot table", "couldn't load advancement", "failed to parse"])
    {
        return Some(Known::DataParseError);
    }
    if has(&["missing or unsupported mandatory dependenc", "missing mandatory dependenc", "incompatible mods found", "requires mod"]) {
        return Some(Known::MissingDependency);
    }
    if has(&["duplicate mod", "found duplicate mods", "duplicate mods found"]) {
        return Some(Known::DuplicateMod);
    }
    if has_type(&["OutOfMemoryError"]) {
        return Some(Known::OutOfMemory);
    }
    if has_type(&["NoClassDefFoundError", "ClassNotFoundException", "NoSuchMethodError", "NoSuchFieldError", "AbstractMethodError", "IncompatibleClassChangeError"])
    {
        return Some(Known::ClassMissing);
    }
    if has(&["failed to create mod instance", "modloadingexception", "mod loading failures have occurred", "encountered an error during the"]) {
        return Some(Known::ModConstruction);
    }
    if has_type(&["ConcurrentModificationException"]) {
        return Some(Known::ConcurrentModification);
    }
    if has_type(&["NullPointerException"]) {
        return Some(Known::NullPointer);
    }
    None
}

fn detect_system(e: &Entry) -> System {
    if matches!(e.source, Source::Gradle | Source::Compiler) {
        return System::Build;
    }
    let logger = e.logger.as_deref().unwrap_or("").to_lowercase();
    // Сообщения исключений сюда не входят: в них имена классов вроде `net.minecraft.world.item.Item`,
    // и слово «world» из пакета ничего не говорит о системе. Типы исключений — входят.
    let mut text = format!("{logger}\n{}", e.message.to_lowercase());
    for d in &e.details {
        text.push('\n');
        text.push_str(&d.to_lowercase());
    }
    for t in e.throwable.iter().flat_map(|t| t.chain()) {
        text.push('\n');
        text.push_str(&t.type_name.to_lowercase());
    }
    let has = |needles: &[&str]| needles.iter().any(|n| text.contains(n));
    let loader_logger = ["net.neoforged.fml", "net.neoforged.neoforgespi", "cpw.mods", "fabricloader", "net.fabricmc.loader"];
    if has(&["mixin", "org.spongepowered"]) {
        System::Mixin
    } else if loader_logger.iter().any(|p| logger.starts_with(p)) || has(&["mods.toml", "fabric.mod.json", "mod loading", "modloading"]) {
        System::ModLoading
    } else if e.thread.as_deref().is_some_and(|t| t.starts_with("modloading-worker")) {
        // Конструктор мода и события загрузки идут в этих потоках.
        System::ModLoading
    } else if has(&["sound"]) {
        System::Sound
    } else if has(&["shader", "rendersystem", "blaze3d", "opengl", "glfw", "render"]) {
        System::Rendering
    } else if has(&["model", "texture", "atlas", "blockstate", "resource pack", "resourcepack", "language", "font"]) {
        System::Resources
    } else if has(&["recipe", "loot", "advancement", "datapack", "data pack", "tagloader", " tag "]) {
        System::Data
    } else if has(&["registr", "deferredregister"]) {
        System::Registries
    } else if has(&["network", "packet", "payload", "connection", "netty"]) {
        System::Network
    } else if has(&["config"]) {
        System::Config
    } else if has(&["chunk", "dimension", "biome", "structure", "world", "level"]) {
        System::World
    } else {
        system_by_frames(e).unwrap_or(System::Game)
    }
}

/// Пакеты Minecraft и загрузчиков → система игры (более точные префиксы — раньше).
const PACKAGE_SYSTEMS: &[(&str, System)] = &[
    ("net.minecraft.client.renderer.block.model", System::Resources),
    ("net.minecraft.client.resources", System::Resources),
    ("net.minecraft.server.packs", System::Resources),
    ("net.minecraft.client.renderer", System::Rendering),
    ("com.mojang.blaze3d", System::Rendering),
    ("net.minecraft.client.gui", System::Rendering),
    ("net.minecraft.client.sounds", System::Sound),
    ("net.minecraft.sounds", System::Sound),
    ("net.minecraft.network", System::Network),
    ("net.minecraft.world.item.crafting", System::Data),
    ("net.minecraft.world.level.storage.loot", System::Data),
    ("net.minecraft.advancements", System::Data),
    ("net.minecraft.tags", System::Data),
    ("net.minecraft.core.registries", System::Registries),
    ("net.minecraft.core.Registry", System::Registries),
    ("net.minecraft.core.MappedRegistry", System::Registries),
    ("net.neoforged.neoforge.registries", System::Registries),
    ("net.fabricmc.fabric.api.event.registry", System::Registries),
    ("net.neoforged.neoforge.network", System::Network),
    ("net.neoforged.fml", System::ModLoading),
    ("net.fabricmc.loader", System::ModLoading),
    ("net.minecraft.world.level", System::World),
    ("net.minecraft.server.level", System::World),
    ("net.minecraft.world.entity", System::World),
];

/// Система по кадрам стека: первый кадр не из своего мода и не из JDK.
fn system_by_frames(e: &Entry) -> Option<System> {
    let t = e.throwable.as_ref()?;
    t.root_cause()
        .frames
        .iter()
        .chain(t.chain().flat_map(|c| c.frames.iter()))
        .filter(|f| !matches!(f.origin, Origin::OwnMod | Origin::Java))
        .find_map(|f| PACKAGE_SYSTEMS.iter().find(|(p, _)| f.class.starts_with(p)).map(|(_, s)| *s))
}

/// «from mod railways» в тексте миксина.
static FROM_MOD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"from mod ([a-z][a-z0-9_]*)").expect("регулярное выражение мода миксина"));
/// Конфигурация миксинов: `railways-common.mixins.json`.
static MIXIN_CONFIG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([a-z0-9_.-]+\.mixins\.json)").expect("регулярное выражение конфигурации миксинов"));

/// Владелец проблемы миксина: мод, названный в тексте, или хозяин конфигурации миксинов.
fn mixin_owner(e: &Entry, rules: &OriginRules) -> Option<Origin> {
    let text = e.search_text();
    if let Some(c) = FROM_MOD.captures(&text) {
        return Some(rules.namespace(&c[1]));
    }
    let config = MIXIN_CONFIG.captures(&text)?;
    rules.mixin_config(&config[1])
}

/// Проблема ресурса или данных: её владелец — тот, чей это ресурс, а не код, который
/// её заметил (текстуру мода не находит Minecraft).
fn is_resource_problem(e: &Entry) -> bool {
    matches!(
        e.known,
        Some(
            Known::MissingTexture
                | Known::MissingModel
                | Known::BlockstateError
                | Known::InvalidResourceLocation
                | Known::InvalidResourcePath
                | Known::DataParseError
                | Known::RegistryNotBound
                | Known::ObjMaterial
                | Known::ObjFile
        )
    ) || matches!(e.system, System::Resources | System::Data)
}

/// Пространство имён ресурса из текста записи (сообщение, продолжение, сообщения причин).
/// Чужое для Minecraft пространство важнее: сломанный ресурс мода ссылается и на ванильные.
fn resource_namespace(e: &Entry) -> Option<String> {
    let mut texts: Vec<&str> = std::iter::once(e.message.as_str()).chain(e.details.iter().map(String::as_str)).collect();
    if let Some(t) = &e.throwable {
        texts.extend(t.chain().filter_map(|c| c.message.as_deref()));
    }
    // `railways-common.mixins.json:client.Foo` — это миксин, а не id ресурса.
    let strict: Vec<String> = texts.iter().flat_map(|t| RESOURCE_ID.captures_iter(t).map(|c| c[1].to_owned())).filter(|ns| !ns.ends_with(".json")).collect();
    let found = if strict.is_empty() && matches!(e.known, Some(Known::InvalidResourceLocation | Known::InvalidResourcePath)) {
        texts.iter().flat_map(|t| ANY_ID.captures_iter(t).map(|c| c[1].to_owned())).collect()
    } else {
        strict
    };
    found.iter().find(|ns| *ns != "minecraft").or(found.first()).cloned()
}

/// Чья это проблема. Свой код в стек-трейсе важнее всего: даже если упало внутри
/// Minecraft, начиналось всё в моде. У проблем ресурсов владелец — пространство имён.
fn detect_origin(e: &Entry, rules: &OriginRules) -> Origin {
    if e.source == Source::Compiler {
        return Origin::OwnMod;
    }
    if let Some(t) = &e.throwable
        && t.chain().flat_map(|c| c.frames.iter()).any(|f| f.origin == Origin::OwnMod)
    {
        return Origin::OwnMod;
    }
    if is_resource_problem(e)
        && let Some(ns) = resource_namespace(e)
    {
        return rules.namespace(&ns);
    }
    // Миксин: «… from mod railways» или имя конфигурации миксинов мода.
    if let Some(o) = mixin_owner(e, rules) {
        return o;
    }
    if let Some(t) = &e.throwable
        && let Some(f) = t.chain().flat_map(|c| c.frames.iter()).find(|f| !matches!(f.origin, Origin::Java | Origin::Library))
    {
        return f.origin.clone();
    }
    let ids = std::iter::once(e.message.as_str()).chain(e.details.iter().map(String::as_str));
    for text in ids {
        if let Some(c) = RESOURCE_ID.captures(text) {
            return rules.namespace(&c[1]);
        }
    }
    if let Some(logger) = &e.logger {
        // Логгер часто называется по классу: `com.example.mymod.MyMod`.
        if logger.contains('.') {
            return rules.class(logger);
        }
    }
    if let Some(f) = e.throwable.as_ref().and_then(|t| t.frames.first()) {
        return f.origin.clone();
    }
    Origin::Unknown
}

/// Через кого проблема всплыла: кто о ней написал (логгер) или где она вылетела
/// (верхний кадр стека).
fn detect_via(e: &Entry, rules: &OriginRules) -> Option<Origin> {
    if matches!(e.source, Source::Compiler | Source::Gradle) {
        return None;
    }
    let by_logger = e.logger.as_deref().filter(|l| l.contains('.')).map(|l| rules.class(l));
    let by_frame = e.throwable.as_ref().and_then(|t| t.frames.iter().find(|f| f.origin != Origin::Java)).map(|f| f.origin.clone());
    by_logger.or(by_frame).filter(|o| !matches!(o, Origin::Unknown | Origin::Java))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Level;

    fn rules() -> OriginRules {
        OriginRules { mod_ids: vec!["testmod".into()], packages: vec!["com.holo.testmod".into()], ..OriginRules::default() }
    }

    fn entry(msg: &str) -> Entry {
        Entry::new(1, Level::Warn, Source::Game, msg)
    }

    #[test]
    fn known_problems_by_message() {
        let cases = [
            ("Attempted to load class net/minecraft/client/Minecraft for invalid dist DEDICATED_SERVER", Known::ClientClassOnServer),
            ("Mixin apply failed testmod.mixins.json:FooMixin -> net.minecraft.Foo", Known::MixinFailure),
            ("Non [a-z0-9/._-] character in path of location: testmod:Ruby", Known::InvalidResourceLocation),
            ("Registry Object not present: testmod:ruby", Known::RegistryNotBound),
            ("Couldn't parse data file testmod:ruby_block from testmod:recipe/ruby_block.json", Known::DataParseError),
            ("Exception loading blockstate definition: 'testmod:blockstates/ruby_block.json' missing model for variant", Known::BlockstateError),
            ("Found duplicate mods: testmod", Known::DuplicateMod),
        ];
        for (msg, want) in cases {
            let mut e = entry(msg);
            classify(&mut e, &rules());
            assert_eq!(e.known, Some(want), "{msg}");
            assert_eq!(e.system, want.system());
        }
    }

    #[test]
    fn known_problems_by_type() {
        let mut e = entry("Exception caught");
        let mut t = Throwable::new("java.lang.RuntimeException", None);
        t.cause = Some(Box::new(Throwable::new("java.util.ConcurrentModificationException", None)));
        e.throwable = Some(t);
        classify(&mut e, &rules());
        assert_eq!(e.known, Some(Known::ConcurrentModification));
    }

    #[test]
    fn systems_by_logger_and_text() {
        let mut e = entry("Failed to create shader program");
        classify(&mut e, &rules());
        assert_eq!(e.system, System::Rendering);
        let mut e = entry("Something");
        e.logger = Some("net.neoforged.fml.ModLoader".into());
        classify(&mut e, &rules());
        assert_eq!(e.system, System::ModLoading);
        assert_eq!(e.origin, Origin::NeoForge, "по имени логгера");
        let mut e = entry("Unknown custom packet identifier: testmod:sync");
        classify(&mut e, &rules());
        assert_eq!(e.system, System::Network);
        assert_eq!(e.origin, Origin::OwnMod);
    }

    #[test]
    fn vanilla_noise_is_marked_but_own_problems_are_not() {
        let mut e = entry("Ambiguity between arguments [teleport, location] and [teleport, destination] with inputs: [0.1 -0.5 .9, 0 0 0]");
        e.logger = Some("net.minecraft.commands.Commands".into());
        classify(&mut e, &rules());
        assert!(e.noise);
        let mut e = entry("Missing sound for event: minecraft:item.goat_horn.play");
        classify(&mut e, &rules());
        assert!(e.noise);
        let mut e = entry("Missing sound for event: testmod:ruby.chime");
        classify(&mut e, &rules());
        assert!(!e.noise, "звук своего мода — настоящая проблема");
        let mut e = entry("Missing textures in model testmod:ruby#inventory:");
        classify(&mut e, &rules());
        assert!(!e.noise);
    }

    #[test]
    fn npe_in_mod_constructor_is_mod_loading() {
        let mut e = Entry::new(1, Level::Error, Source::Game, "Не удалось описать рубин");
        e.thread = Some("modloading-worker-0".into());
        let mut t = Throwable::new("java.lang.NullPointerException", Some("Cannot invoke \"Item.getDescriptionId()\" because \"item\" is null".into()));
        t.frames.push(crate::model::Frame {
            module: None,
            class: "com.holo.testmod.Gems".into(),
            method: "describe".into(),
            file: Some("Gems.java".into()),
            line: Some(10),
            jar: None,
            origin: Origin::OwnMod,
        });
        e.throwable = Some(t);
        classify(&mut e, &rules());
        assert_eq!(e.known, Some(Known::NullPointer));
        assert_eq!(e.system, System::ModLoading);
        assert_eq!(e.origin, Origin::OwnMod);
    }

    #[test]
    fn package_words_in_exception_messages_do_not_pick_the_system() {
        // В сообщении — `net.minecraft.world.item`, но это не про мир.
        let mut e = Entry::new(1, Level::Error, Source::Game, "Exception ticking");
        e.thread = Some("Render thread".into());
        let mut t = Throwable::new("java.lang.NullPointerException", Some("Cannot invoke \"net.minecraft.world.item.Item.getDescriptionId()\"".into()));
        for class in ["com.holo.testmod.Gems", "net.minecraft.client.renderer.ItemRenderer", "net.minecraft.client.Minecraft"] {
            t.frames.push(crate::model::Frame {
                module: None,
                class: class.into(),
                method: "m".into(),
                file: None,
                line: None,
                jar: None,
                origin: rules().class(class),
            });
        }
        e.throwable = Some(t);
        classify(&mut e, &rules());
        assert_eq!(e.system, System::Rendering, "по первому кадру Minecraft");
    }

    fn frame(class: &str, origin: Origin) -> crate::model::Frame {
        crate::model::Frame { module: None, class: class.into(), method: "m".into(), file: None, line: None, jar: None, origin }
    }

    #[test]
    fn resource_namespace_beats_the_frames_that_noticed_it() {
        // Модель Steam 'n' Rails не загрузилась внутри кода Minecraft.
        let mut e = Entry::new(1, Level::Warn, Source::Game, "Unable to load model: 'railways:block/bogey' referenced from: minecraft:stone");
        e.logger = Some("net.minecraft.client.resources.model.ModelBakery".into());
        let mut t = Throwable::new("java.lang.RuntimeException", Some("Unable to load model railways:block/bogey".into()));
        t.frames.push(frame("net.minecraft.client.resources.model.ModelBakery", Origin::Minecraft));
        e.throwable = Some(t);
        classify(&mut e, &rules());
        assert_eq!(e.origin, Origin::OtherMod(Some("railways".into())));
        assert_eq!(e.via, Some(Origin::Minecraft), "заметил Minecraft");
    }

    #[test]
    fn explanation_comes_from_the_deepest_cause() {
        let mut e = Entry::new(1, Level::Error, Source::Game, "Failed to load model testmod:item/ruby");
        let mut t = Throwable::new("java.lang.RuntimeException", Some("Unable to load model testmod:item/ruby".into()));
        t.cause = Some(Box::new(Throwable::new("java.util.NoSuchElementException", Some("The material was not found in the library: ruby_mat".into()))));
        e.throwable = Some(t);
        classify(&mut e, &rules());
        assert_eq!(e.known, Some(Known::ObjMaterial), "а не «модель не найдена»");
        assert_eq!(e.system, System::Resources);
        assert_eq!(e.origin, Origin::OwnMod);
    }

    #[test]
    fn invalid_ids_name_their_mod() {
        let mut e = entry("Non [a-z0-9/._-] character in path of location: railways:Bad Name");
        classify(&mut e, &rules());
        assert_eq!(e.known, Some(Known::InvalidResourceLocation));
        assert_eq!(e.origin, Origin::OtherMod(Some("railways".into())));
        let mut e = entry("See https://example.com/x for Non [a-z0-9/._-] character in path of location");
        classify(&mut e, &rules());
        assert_ne!(e.origin, Origin::OtherMod(Some("https".into())), "адрес — не id");
    }

    #[test]
    fn mod_loading_failure_surfaces_via_the_loader() {
        let mut e = entry("Failed to create mod instance. ModID: testmod");
        e.logger = Some("net.neoforged.fml.ModContainer".into());
        let mut t = Throwable::new("java.lang.NullPointerException", None);
        t.frames.push(frame("com.holo.testmod.TestMod", Origin::OwnMod));
        e.throwable = Some(t);
        classify(&mut e, &rules());
        assert_eq!(e.origin, Origin::OwnMod);
        assert_eq!(e.via, Some(Origin::NeoForge));
        assert_eq!(e.known, Some(Known::NullPointer), "по самой глубокой причине");
    }

    #[test]
    fn mixin_problems_belong_to_their_mod() {
        let mut e = entry("@Mixin target net.minecraft.client.gui.screens.controls.KeyBindsList$KeyEntry was not found railways-common.mixins.json:client.MixinKeyBindsListKeyEntry from mod railways");
        e.logger = Some("mixin".into());
        classify(&mut e, &rules());
        assert_eq!(e.origin, Origin::OtherMod(Some("railways".into())));
        let mut e = entry("Mixin apply failed testmod.mixins.json:FooMixin -> net.minecraft.Foo");
        classify(&mut e, &rules());
        assert_eq!(e.origin, Origin::OwnMod);
    }

    #[test]
    fn invalid_paths_in_packs_are_invalid_ids() {
        let mut e = Entry::new(1, Level::Error, Source::Game, "Invalid path in pack: railways:models/block/bogey/wide/scotch_yoke/Broad_Gauge_L_0-2-0.bbmodel, ignoring");
        e.logger = Some("net.minecraft.Util".into());
        classify(&mut e, &rules());
        assert_eq!(e.known, Some(Known::InvalidResourcePath));
        assert_eq!(e.system, System::Resources);
        assert_eq!(e.origin, Origin::OtherMod(Some("railways".into())));
        assert_eq!(e.via, Some(Origin::Minecraft));
    }

    #[test]
    fn times_are_not_resource_ids() {
        let mut e = entry("Took 00:12:40 to load");
        e.logger = Some("Foo".into());
        classify(&mut e, &rules());
        assert_eq!(e.origin, Origin::Unknown);
    }
}
