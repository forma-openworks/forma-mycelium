# Hyphae, the knowledge engine

[English](#english) · [Русский](#русский)

---

## English

Hyphae is the part of Forma Mycelium that understands a Minecraft mod as a whole, not just as Java code. A regular IDE sees classes and files. Hyphae also sees mods, loaders, resource namespaces, mixins and the game itself, and answers the questions a modder actually asks:

- *Whose problem is this?* A missing texture can be noticed by Minecraft but belong to Steam 'n' Rails.
- *What is actually in my run?* Which mods, which versions, where each of them came from.
- *Why won't the game start?* A required dependency is missing, or a mod is in the run twice.

The name comes from hyphae, the threads a mycelium is made of: they reach into everything and carry information back.

### How it is built

Hyphae is an ordinary Rust library, the `mycelium-hyphae` crate. It has no UI of its own: Forma Mycelium calls it and shows the results. It is built in three layers.

1. **Readers** turn files into data:
   - jars: entries, the manifest, the JPMS module name from `module-info.class` or `Automatic-Module-Name`;
   - mod metadata: `META-INF/neoforge.mods.toml`, `META-INF/mods.toml`, `fabric.mod.json`, `quilt.mod.json`;
   - nested jars (jar-in-jar): `META-INF/jarjar/metadata.json` on NeoForge and Forge, `jars` on Fabric and Quilt.
2. **The index** keeps what is known about the project and the run.
3. **Analyzers** turn the index into conclusions, such as "this class belongs to Create" or "this log line is a problem of my mod".

### What works today

#### The mods of the run

Hyphae collects everything that goes into the game:

| Source | What it is |
|---|---|
| The project | Your own mod: id, name, version, logo and packages. |
| Gradle | The run classpath (`runtimeClasspath`), so mods added with `runtimeOnly` or `localRuntime` are included. |
| `run/mods` | Jars dropped into the run folder, for the root project and for every subproject. |
| Nested jars | Mods and libraries inside other jars, up to three levels deep. |

For every mod it records:
- the id, display name, version, authors and description;
- the logo, with fallbacks when the metadata points to a missing file;
- required and optional dependencies, mixin configs, packages and resource namespaces.

The Minecraft jar of ModDevGradle also carries NeoForge, so it is split into two entries. On a real project with Create, Flywheel, Ponder and Steam 'n' Rails, about 90 jars are read in 0.1–0.2 seconds.

Two problems are found before the game even fails:
- **A mod in the run twice.** Only top-level copies count: duplicates inside nested jars are resolved by the loader itself.
- **A missing required dependency.** For example, a mod that needs Create when Create is not in the run.

#### Whose code is this

From the mods of the run Hyphae builds a map that the console and the exceptions window use:
- **Packages → mod.** `com.simibubi.create.content.Foo` belongs to Create, not to "some library".
- **JPMS modules → mod.** Stack frames such as `TRANSFORMER/create@6.0.10/…` name their mod. A module with dots in its name (`com.google.gson`) is never treated as a mod: mod ids have no dots.
- **Resource namespaces → mod.** If a namespace is not a mod id, it belongs to the mod that ships it, as long as only one mod does. For example, `byg:…` files inside the Steam 'n' Rails jar belong to Steam 'n' Rails.
- **Mixin configs → mod.** `railways-common.mixins.json` belongs to Steam 'n' Rails.
- **Shortened logger names.** The NeoForge console shortens them (`[minecraft/Util]`, `ne.ne.fm.ModLoader`), and Hyphae expands them to the full names (`net.minecraft.Util`, `net.neoforged.fml.ModLoader`). This way the console understands lines as well as `latest.log` does.

#### Two roles of a problem

Every problem in the console and the exceptions window has:
- **an owner**, whose problem it is and who should fix it;
- **the one it surfaced via**: who noticed or reported it.

A broken model of Steam 'n' Rails is shown as *Create: Steam 'n' Rails (railways) via Minecraft*.

The rules for the owner, in order:
1. If your code is in the stack trace, the problem is yours.
2. A resource problem belongs to the namespace of the resource id, not to the code that noticed it.
3. A mixin problem belongs to the mod named in it ("from mod X") or to the owner of its mixin config.
4. Otherwise the first stack frame that isn't the JDK or a library decides, then the resource id in the message, then the logger.

The plain-language explanation is based on the deepest cause in the chain. A wrapper such as "Unable to load model" only says where things broke. The root cause, such as "The material was not found in the library", says what broke.

### Where you see it

- **Mods of the run.** A tab in the Debug workspace lists your mod, Minecraft and the loader, other mods and libraries.
  - Each mod shows its logo, version, origin, dependencies (present or missing), mixin configs and problem counters.
  - Clicking a problem opens it in the exceptions window.
- **The exceptions window and the console.**
  - Mod tags show the mod's name, id and logo. Your own mod's tag is lime; other mods share one color and are told apart by their logos.
  - Every problem shows its owner and, when they differ, the one it surfaced via.
  - Messages are marked up: resource ids are colored by their owner, invalid characters in ids are highlighted, classes and files are picked out.

### Roadmap

1. ✅ **The mods of the run and ownership** of classes, resources and log lines.
2. **Bytecode index:**
   - class hierarchy;
   - which classes exist only on the client, found by comparing the client and server jars;
   - kind icons in the project tree (Block, Item, BlockEntity…);
   - origin and side badges in code completion.
3. **Source analysis:**
   - logical sides (`isClientSide`, `ServerLevel` / `ClientLevel`), threads and loading stages;
   - an error for client code reachable on a dedicated server, with a "build anyway" escape hatch.
4. **The version atlas.** The Minecraft and loader API across versions, computed automatically: renames are found through Fabric intermediary names, and hand-written notes explain big system changes. It powers a *Porting* panel.
5. **Mixins and the mod map:**
   - "what Mixin sees", gutter navigation, a generator that prefers MixinExtras;
   - validation with quick fixes and a conflict radar;
   - a node graph of the whole mod.

Knowledge about Minecraft itself will live in data packs (TOML) per Minecraft and loader version. They will ship inside the app and can be overridden by the user. An API for third-party knowledge packs (for example, for Create add-ons) is planned for later. The design keeps room for it.

### For developers

| Path | What it holds |
|---|---|
| `crates/mycelium-hyphae/src/jar.rs` | Reading jars: entries, manifest, module names, automatic module names |
| `crates/mycelium-hyphae/src/meta.rs` | Mod metadata of every loader and the lists of nested jars |
| `crates/mycelium-hyphae/src/mods.rs` | `RunMods`: the mods of the run and the ownership map |
| `crates/mycelium-project/src/origin.rs` | `OriginRules` and `ModMap`: origin of classes, modules, namespaces, mixin configs and loggers |
| `crates/mycelium-log/src/classify.rs` | Owner, "surfaced via", game system and known problems of a log entry |

To print the mods of the run of any project, as Hyphae sees them:

```
cargo run -p mycelium-hyphae --example run_mods -- <mod folder> [extra jar…]
```

The classpath is taken from `.mycelium/cache/model.json`. Jars given as arguments are added to it.

---

## Русский

Hyphae — часть Forma Mycelium, которая понимает мод для Minecraft целиком, а не только как код на Java. Обычная IDE видит классы и файлы. Hyphae видит ещё моды, загрузчики, пространства имён ресурсов, миксины и саму игру, и отвечает на вопросы, которые моддер задаёт на самом деле:

- *Чья это проблема?* Пропавшую текстуру может заметить Minecraft, а принадлежать она — Steam 'n' Rails.
- *Что на самом деле в моём запуске?* Какие моды, каких версий и откуда каждый взялся.
- *Почему игра не запускается?* Нет обязательной зависимости или мод лежит в запуске дважды.

Название — от гиф (hyphae), нитей, из которых состоит мицелий: они прорастают во всё вокруг и приносят обратно сведения.

### Как устроен

Hyphae — обычная библиотека на Rust, крейт `mycelium-hyphae`. Своего интерфейса у неё нет: Forma Mycelium вызывает её и показывает результаты. Она состоит из трёх слоёв.

1. **Читалки** превращают файлы в данные:
   - jar: список файлов, манифест, имя модуля JPMS из `module-info.class` или `Automatic-Module-Name`;
   - метаданные модов: `META-INF/neoforge.mods.toml`, `META-INF/mods.toml`, `fabric.mod.json`, `quilt.mod.json`;
   - вложенные jar (jar-in-jar): `META-INF/jarjar/metadata.json` у NeoForge и Forge, `jars` у Fabric и Quilt.
2. **Индекс** хранит то, что известно о проекте и запуске.
3. **Анализаторы** делают из индекса выводы, например «этот класс принадлежит Create» или «эта строка лога — проблема моего мода».

### Что работает уже сейчас

#### Моды запуска

Hyphae собирает всё, что попадает в игру:

| Источник | Что это |
|---|---|
| Проект | Свой мод: id, название, версия, логотип и пакеты. |
| Gradle | Classpath запуска (`runtimeClasspath`), так что попадают и моды, добавленные через `runtimeOnly` или `localRuntime`. |
| `run/mods` | jar, положенные в папку запуска, — у корня и у каждого подпроекта. |
| Вложенные jar | Моды и библиотеки внутри других jar, до трёх уровней вложенности. |

Для каждого мода записывается:
- id, название, версия, авторы и описание;
- логотип, с запасными вариантами, если метаданные ссылаются на несуществующий файл;
- обязательные и необязательные зависимости, конфигурации миксинов, пакеты и пространства имён ресурсов.

jar Minecraft у ModDevGradle несёт в себе и NeoForge, поэтому он делится на две записи. На настоящем проекте с Create, Flywheel, Ponder и Steam 'n' Rails около 90 jar читаются за 0.1–0.2 секунды.

Ещё до того, как игра упадёт, находятся две проблемы:
- **Мод лежит в запуске дважды.** Считаются только копии на верхнем уровне: дубли внутри вложенных jar загрузчик разрешает сам.
- **Нет обязательной зависимости.** Например, моду нужен Create, а Create в запуске нет.

#### Чей это код

Из модов запуска Hyphae строит карту, которой пользуются консоль и окно исключений:
- **Пакеты → мод.** `com.simibubi.create.content.Foo` принадлежит Create, а не «какой-то библиотеке».
- **Модули JPMS → мод.** Кадры стека вида `TRANSFORMER/create@6.0.10/…` называют свой мод. Модуль с точками в имени (`com.google.gson`) никогда не считается модом: в id модов точек не бывает.
- **Пространства имён ресурсов → мод.** Если пространство имён не совпадает с id мода, оно принадлежит моду, который кладёт его в свои ресурсы, если такой мод один. Например, файлы `byg:…` внутри jar Steam 'n' Rails принадлежат Steam 'n' Rails.
- **Конфигурации миксинов → мод.** `railways-common.mixins.json` принадлежит Steam 'n' Rails.
- **Сокращённые имена логгеров.** Консоль NeoForge их сокращает (`[minecraft/Util]`, `ne.ne.fm.ModLoader`), а Hyphae разворачивает в полные (`net.minecraft.Util`, `net.neoforged.fml.ModLoader`). Так консоль понимает строки не хуже, чем `latest.log`.

#### Две роли проблемы

У каждой проблемы в консоли и окне исключений есть:
- **владелец** — чья это проблема и кому её исправлять;
- **тот, через кого она всплыла** — кто её заметил или сообщил о ней.

Сломанная модель Steam 'n' Rails показывается как *Create: Steam 'n' Rails (railways) через Minecraft*.

Владелец определяется по правилам, по порядку:
1. Если в стек-трейсе есть твой код, проблема твоя.
2. Проблема ресурса принадлежит пространству имён id ресурса, а не коду, который её заметил.
3. Проблема миксина принадлежит моду, названному в ней («from mod X»), или хозяину её конфигурации миксинов.
4. Иначе решает первый кадр стека, который не JDK и не библиотека, затем id ресурса в сообщении, затем логгер.

Объяснение простыми словами строится по самой глубокой причине в цепочке. Обёртка вроде «Unable to load model» говорит только, где всё сломалось. Корневая причина, например «The material was not found in the library», говорит, что именно сломалось.

### Где это видно

- **Моды запуска.** Вкладка в пространстве «Отладка» показывает свой мод, Minecraft и загрузчик, чужие моды и библиотеки.
  - У каждого мода видны логотип, версия, откуда он взялся, зависимости (есть или нет), конфигурации миксинов и счётчики проблем.
  - Щелчок по проблеме открывает её в окне исключений.
- **Окно исключений и консоль.**
  - На плашках модов — название, id и логотип мода. Плашка своего мода салатовая, чужие — одного цвета, и их различают по логотипу.
  - У каждой проблемы показан владелец и, если отличается, тот, через кого она всплыла.
  - Сообщения размечены: id ресурсов окрашены цветом владельца, недопустимые символы в id подсвечены, классы и файлы выделены.

### Дальнейший план

1. ✅ **Моды запуска и принадлежность** классов, ресурсов и строк лога.
2. **Индекс байткода:**
   - иерархия классов;
   - какие классы есть только у клиента — по сравнению jar клиента и сервера;
   - значки видов в дереве проекта (Block, Item, BlockEntity…);
   - пометки происхождения и стороны в автодополнении.
3. **Разбор исходников:**
   - логические стороны (`isClientSide`, `ServerLevel` / `ClientLevel`), потоки и этапы загрузки;
   - ошибка, если клиентский код достижим на выделенном сервере, с выходом «всё равно собрать».
4. **Атлас версий.** API Minecraft и загрузчика по всем версиям, вычисленный автоматически: переименования находятся через промежуточные имена Fabric, а ручные заметки объясняют крупные изменения систем. На нём работает панель *Портирование*.
5. **Миксины и карта мода:**
   - «что видит Mixin», навигация с полей редактора, генератор с MixinExtras по умолчанию;
   - проверки с быстрыми исправлениями и радар конфликтов;
   - граф всего мода.

Знания о самом Minecraft будут жить в пакетах данных (TOML) для каждой версии Minecraft и загрузчика. Они будут встроены в программу, и пользователь сможет их переопределить. API для сторонних пакетов знаний (например, для дополнений к Create) запланирован на потом. Устройство движка оставляет для него место.

### Разработчикам

| Путь | Что там |
|---|---|
| `crates/mycelium-hyphae/src/jar.rs` | Чтение jar: файлы, манифест, имена модулей, автоматические имена модулей |
| `crates/mycelium-hyphae/src/meta.rs` | Метаданные модов всех загрузчиков и списки вложенных jar |
| `crates/mycelium-hyphae/src/mods.rs` | `RunMods`: моды запуска и карта принадлежности |
| `crates/mycelium-project/src/origin.rs` | `OriginRules` и `ModMap`: происхождение классов, модулей, пространств имён, конфигураций миксинов и логгеров |
| `crates/mycelium-log/src/classify.rs` | Владелец, «через кого», система игры и известные проблемы записи лога |

Чтобы вывести моды запуска любого проекта так, как их видит Hyphae:

```
cargo run -p mycelium-hyphae --example run_mods -- <папка мода> [ещё jar…]
```

Classpath берётся из `.mycelium/cache/model.json`. jar из аргументов добавляются к нему.
