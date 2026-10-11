<p align="center">
  <img src="assets/discord/mycelium.png" width="128" alt="Forma Mycelium logo">
</p>

<h1 align="center">Forma Mycelium</h1>

<p align="center">
  <b>An IDE made for Minecraft modding.</b><br>
  NeoForge and Fabric · Minecraft 1.21.1+ · Java and Kotlin
</p>

<p align="center">
  <img alt="Status: early development" src="https://img.shields.io/badge/status-early%20development-8a6cf0">
  <img alt="Rust 2024" src="https://img.shields.io/badge/Rust-2024-f46623?logo=rust&logoColor=white">
  <img alt="Rendering: wgpu + Vulkan" src="https://img.shields.io/badge/render-wgpu%20%2B%20Vulkan-a41e22">
  <img alt="Languages: English, Russian" src="https://img.shields.io/badge/UI-English%20%7C%20%D0%A0%D1%83%D1%81%D1%81%D0%BA%D0%B8%D0%B9-4b8bbe">
</p>

<p align="center">
  <a href="#english">English</a> · <a href="#русский">Русский</a>
</p>

---

## English

### What it is

Forma Mycelium is a desktop IDE in the spirit of IntelliJ IDEA and Rider, built for one job: making Minecraft mods.

A general-purpose Java IDE sees a mod as abstract Java. A stack trace is just a stack trace, and your classes sit in one pile with Minecraft's, the loader's and every other mod's. Mycelium reads a project the way a modder does:
- it knows which code belongs to your mod, Minecraft, the mod loader, another mod or a plain library;
- it explains problems in the game's own terms: which game system broke, how serious it is, and whose code is behind it.

The name comes from mycelium, the network that quietly connects everything around it. It is also a Minecraft block.

### Who it is for

- **Modders who are still learning.** An error should come with an explanation, not just a wall of red text.
- **Experienced modders** who want their own code clearly separated from everything else, and a console that filters out noise.
- **Anyone who works on NeoForge and Fabric** for Minecraft 1.21.1 and newer, in Java or Kotlin.

### How it differs from a general Java IDE

| | General Java IDE | Forma Mycelium |
|---|---|---|
| **An exception** | A stack trace | The game system involved (resources, registries, mixins, rendering…), the severity, whose code it is, and a plain-language explanation of known errors |
| **Dependencies** | A flat list of jars | Grouped by origin: Minecraft, the loader, other mods, libraries, JDK. Each one opens with its sources |
| **The game log** | Raw text | Classified and filtered. Harmless dev-environment noise is marked and not counted as a problem |
| **Running the game** | One run at a time | Client, server and data generation side by side, each with its own log, and all their problems collected in one place |
| **Project settings** | Scattered across Gradle and IDE files | A hidden `.mycelium` folder, like `.idea`. The mod folder stays clean |

### What already works

**Understands your mod**
- **Mod detection:** NeoForge, Fabric, Forge, Quilt and multi-loader projects, plus the Minecraft and loader versions, the mod id, Java and Kotlin.
- **The project model comes from Gradle itself.** A small init script exports modules, dependencies with their jar paths, the toolchain JDK and the real `run*` tasks. Nothing is written into the mod.
- **External libraries by origin.** Minecraft classes open as read-only sources, straight from `-sources.jar`.

**Runs and diagnoses**
- **Run configurations** are built from the real Gradle tasks: client, server, data generation, build, clean, plus your own tasks and arguments. They are stored in `.mycelium/runs.json`, which you can commit.
- **Concurrent runs:** for example, a client and a server to test multiplayer, each in its own log tab.
- **A smart console** that sorts every line by severity, game system and origin, with filters and search. Foreign stack frames are folded.
- **An exceptions window** that gathers every problem from every run, with repeat counters and grouping. Clicking a frame opens the exact line, in your code or in Minecraft's and NeoForge's sources.
  - Every problem has an owner, whose problem it is, and the one it surfaced via: a broken texture of Steam 'n' Rails shows as "Create: Steam 'n' Rails (railways) via Minecraft".
  - Mods are shown with their names and logos. Your own mod's tag is lime.
  - Messages are marked up: resource ids colored by their owner, invalid characters in ids highlighted, classes and files picked out.
  - The plain-language explanation is based on the deepest cause in the chain, not on the wrapper on top.
- **Mods of the run** lists everything that goes into the game: your mod, Minecraft and the loader, other mods from Gradle and `run/mods`, mods nested inside other jars, and libraries.
  - Each mod shows its logo, version, where it came from, its dependencies and mixin configs.
  - Problems are counted per mod. A missing required dependency or a mod that is in the run twice is flagged before the game even fails.

**Helps write code**
- **A code editor** with syntax highlighting and autosave.
- **Java assistance through jdtls:**
  - errors underlined as you type;
  - completion that adds imports;
  - documentation on hover;
  - go to definition (`Ctrl+click`, `F12`, `Ctrl+B`), even into Minecraft's code.

  The language server is an optional plugin, installed with one click when you want it.

**Feels like home**
- **A layout in the spirit of Blender.** Any area splits into tabs. Tabs can be dragged between areas or torn out into separate OS windows.
- **A window frame of its own.** The Windows title bar is replaced by the app's top bar with minimize, maximize and close buttons. Drag the bar to move the window, double-click it to maximize.
- **Workspaces** (Code, Map, Assets, Debug), each with its own layout. A layout can be saved per project, or as the default for all projects.
- **Discord Rich Presence:** "Mod: Your Mod (yourmod)" with a live status (writing code, running the client, fixing a crash…), a timer and the loader badge. It can be turned off, and a project can be marked secret.
- **English and Russian UI,** switchable on the fly, plus **English · Russian hints**: menus and terms stay in English, as in modding guides, while hints and explanations are in Russian. Extra languages can be added as Fluent files.
- **Themes where everything can be restyled:**
  - default themes Mycelium Dark and Mycelium Light (marked "Default" in theme lists), plus Visual Studio Dark as a one-click theme plugin;
  - your own themes are `.toml` files that override only what they change, so a pink menu bar is literally one line (`"menubar.bg" = "#ff69b4"`). They reload as soon as you save them;
  - a theme sets every region (menu bar, status bar, tabs, editor, popups, buttons, console, origin colors), code highlighting with bold and italic, and optionally its own fonts and background picture;
  - **Quick switch theme** (``Ctrl+` ``): hover or use the arrows to preview, Enter keeps it, Esc brings the old one back;
  - **icons** come from the [Lucide](https://lucide.dev) set. A theme can replace any icon with its own SVG and recolor any icon with an `icon.<name>` key;
  - **tools for theme authors:** *Create theme* writes a new theme file that lists every key, highlight class and icon, and the *Theme inspector* shows them all with their current values.
- **Fonts:** separate fonts for the interface, the code editor and the console, picked from the fonts installed on your system, with size in pixels and line height. `Ctrl+mouse wheel` resizes the editor and console font in half-pixel steps.
- **A background picture,** drawn semi-transparent over the window, like in Rider.
- **Narrow areas keep working.** Text is shortened with an ellipsis, toolbars wrap or fold their filters into a menu, extra tabs and workspaces move into a » menu, and dialogs scroll instead of overflowing the window.
- **Plugins.** Anything big and optional is a plugin: the Java language server and the Visual Studio Dark theme now, Kotlin and Python later. The Plugins page has Recommended and Installed tabs, one-click installs with progress, an on/off switch, and theme packs installed from a `.zip`. Recommended plugins are also offered on the welcome screen.
- **Settings in the spirit of JetBrains IDEs:**
  - a settings window with a page tree and search across every option;
  - changes are applied with Apply or OK and discarded with Cancel;
  - a keymap where every command can be rebound, with conflicts shown as you record a shortcut;
  - **Find Action** (`Ctrl+Shift+A`);
  - export and import of settings by category. An archive from another Forma app applies whatever this app understands and reports the rest.

### Roadmap

1. ✅ **Skeleton:** the composable UI, localization, the build.
2. ✅ **Project and code:** `.mycelium`, the Gradle model, libraries by origin, runs, the smart console, the exceptions window, jdtls.
3. 🔧 **Forma platform (in progress):**
   - ✅ settings grouped as in Rider, export and import;
   - ✅ a keymap and Find Action;
   - ✅ themes where everything can be restyled, three independent fonts, a background picture;
   - ✅ an SVG icon set that themes can replace and recolor, and tools for theme authors;
   - ✅ plugins: a plugin manager, jdtls as an optional plugin, theme packs;
   - Python scripting and new windows from plugins.
4. 🔧 **Hyphae, the knowledge engine (in progress, see [docs/hyphae.md](docs/hyphae.md)).** It understands the whole mod:
   - ✅ the mods of the run and which mod every class, resource and log line belongs to;
   - client and server sides, threads, loading stages;
   - mixins and their targets;
   - what changed between Minecraft and NeoForge versions, and how to port.
5. **The mod map:** a node graph of your classes and resources with automatic links, groups like Blender frames, sticky notes, and markers for what each part depends on.
6. **Assets:** JSON and OBJ model preview, validated data files, language files. Kotlin assistance.

### Built with

- [Rust](https://www.rust-lang.org/) and [egui](https://github.com/emilk/egui), rendered through [wgpu](https://wgpu.rs/) on Vulkan. The UI is drawn by the app itself, with no web view.
- [Fluent](https://projectfluent.org/) for localization.
- [Eclipse JDT Language Server](https://github.com/eclipse-jdtls/eclipse.jdt.ls) for Java assistance.
- [Lucide](https://lucide.dev) icons (ISC License, see `crates/forma-ui/icons/lucide/LICENSE`), drawn with [resvg](https://github.com/linebender/resvg).
- Gradle init scripts for the project model. No Gradle plugin is added to your mod.

### Building

You need stable Rust (edition 2024) and a GPU driver with Vulkan support. Development and testing currently happen on Windows.

```bash
cargo run --release
```

Open a mod right away:

```bash
cargo run --release -- "D:\Mods\my-mod"
```

Build `dist\Forma Mycelium.exe`:

```bash
cargo xtask dist
```

Run the tests:

```bash
cargo test
```

### Shortcuts

The defaults follow IntelliJ IDEA. Every one of them can be changed in **Settings → Keymap**.

| Action | Shortcut |
|---|---|
| Find action | `Ctrl+Shift+A` |
| Quick switch theme | ``Ctrl+` `` |
| Settings | `Ctrl+Alt+S` |
| Run / stop | `Shift+F10` / `Ctrl+F2` |
| Sync with Gradle | `Ctrl+Shift+O` |
| Completion | `Ctrl+Space` (opens by itself after `.`) |
| Go to definition | `Ctrl+click`, `F12` or `Ctrl+B` |
| Project / Console / Exceptions window | `Alt+1` / `Alt+4` / `Alt+6` |
| Maximize an area / restore | `Ctrl+Shift+F12` or double-click a tab |
| Close a tab | Middle-click the tab |
| Save all | `Ctrl+S` (files also save themselves) |
| Zoom | `Ctrl+=` / `Ctrl+-` / `Ctrl+0` |

### Repository layout

The `forma-*` crates are the Forma platform, a base shared by Forma apps. The `mycelium-*` crates are everything specific to modding.

| Path | Contents |
|---|---|
| `crates/mycelium` | The app: window, menus, workspaces, editors, runs, the jdtls client, Discord |
| `crates/mycelium-project` | The mod project: `.mycelium`, detection, the Gradle model, libraries, code origin |
| `crates/mycelium-log` | Parsing and classifying game, Gradle and compiler logs |
| `crates/mycelium-hyphae` | Hyphae, the knowledge engine: jars, mod metadata, nested jars, the mods of the run and who owns what |
| `crates/forma-layout` | The composable UI: area tree, tabs, drag and drop, OS windows |
| `crates/forma-ui` | The theme engine, icons, fonts, widgets, the settings window and its pages, Find Action, theme tools |
| `crates/forma-ui/icons/lucide/` | The Lucide icons used by the app and their license |
| `crates/forma-plugins` | Plugins: the manifest, installing, removing, turning on and off |
| `crates/mycelium/themes/` | Built-in themes |
| `crates/mycelium/plugins/` | Plugins the app carries inside (the Visual Studio Dark theme) |
| `crates/forma-actions` | Actions and the keymap |
| `crates/forma-settings` | Settings components, their files, export and import |
| `crates/forma-i18n` | Localization: Fluent, the `t!` macro, translation completeness checks |
| `locales/<lang>/*.ftl` | UI translations (`platform.ftl` holds the platform's strings) |
| `assets/logo/` | The logo: a detailed version for large sizes, a simple one for small icons |
| `assets/discord/` | Images for Discord Rich Presence (sources and licenses in its README) |
| `xtask` | Build tasks (`cargo xtask dist`, `cargo xtask discord-assets`) |

### Where data lives

- `%APPDATA%\Forma\Mycelium\` stores workspaces, window placement and the default layout. Settings live in `options\`, one file per category (`appearance.json`, `fonts.json`, `keymap.json`…), and your own themes in `themes\`. Set `FORMA_CONFIG_HOME` to keep them somewhere else, for example for a portable copy (plugins and caches then move there too, into `cache\`).
- `%LOCALAPPDATA%\Forma\Mycelium\` stores plugins (`plugins\`, for example the Java language server) and per-project indexes. Eclipse metadata lives here, not in your mod.
- `<mod>\.mycelium\` is the project:
  - `project.json` and `runs.json` are meant for git;
  - `workspace.json` is your personal layout;
  - `cache\` holds the Gradle model.

### About

Forma Mycelium is part of FORMA. Idea and design: Holo. The code is written entirely by Claude (Anthropic).

---

## Русский

### Что это

Forma Mycelium — среда разработки в духе IntelliJ IDEA и Rider, созданная для одной задачи: делать моды для Minecraft.

Обычная Java-IDE видит мод как абстрактную Java. Стек-трейс для неё просто стек-трейс, а твои классы лежат в одной куче с классами Minecraft, загрузчика и всех остальных модов. Mycelium читает проект так, как его читает моддер:
- знает, чей это код: твоего мода, Minecraft, загрузчика, другого мода или обычной библиотеки;
- объясняет проблемы на языке игры: какая система сломалась, насколько это серьёзно и чей код за этим стоит.

Название — от мицелия, сети, которая незаметно связывает всё вокруг. А ещё это блок в Minecraft.

### Для кого

- **Для моддеров, которые только учатся.** Ошибка должна приходить с объяснением, а не стеной красного текста.
- **Для опытных моддеров,** которым нужно чётко отделять свой код от чужого и консоль без шума.
- **Для всех, кто пишет под NeoForge и Fabric** для Minecraft 1.21.1 и новее, на Java или Kotlin.

### Чем отличается от обычной Java-IDE

| | Обычная Java-IDE | Forma Mycelium |
|---|---|---|
| **Исключение** | Стек-трейс | Система игры (ресурсы, реестры, миксины, отрисовка…), серьёзность, чей это код и объяснение известных ошибок простыми словами |
| **Зависимости** | Плоский список jar | По происхождению: Minecraft, загрузчик, другие моды, библиотеки, JDK. Каждая открывается с исходниками |
| **Лог игры** | Сырой текст | Разобран и отфильтрован. Безобидный шум среды разработки помечен и не считается проблемой |
| **Запуск игры** | Один запуск за раз | Клиент, сервер и генерация данных одновременно, у каждого свой лог, а все проблемы собраны в одном месте |
| **Настройки проекта** | Разбросаны по Gradle и файлам IDE | Скрытая папка `.mycelium`, как `.idea`. Папка мода остаётся чистой |

### Что уже работает

**Понимает мод**
- **Распознавание мода:** NeoForge, Fabric, Forge, Quilt и мультилоадерные проекты, версии Minecraft и загрузчика, id мода, Java и Kotlin.
- **Модель проекта берётся у самого Gradle.** Маленький init-скрипт выгружает модули, зависимости с путями к jar, JDK тулчейна и настоящие задачи `run*`. В мод при этом ничего не пишется.
- **Внешние библиотеки по происхождению.** Классы Minecraft открываются исходниками из `-sources.jar` только для чтения.

**Запускает и разбирается**
- **Конфигурации запуска** строятся из настоящих задач Gradle: клиент, сервер, генерация данных, сборка, очистка, плюс свои задачи и аргументы. Хранятся в `.mycelium/runs.json`, его можно класть в git.
- **Несколько запусков одновременно:** например, клиент и сервер для проверки мультиплеера, каждый в своей вкладке лога.
- **Умная консоль** раскладывает каждую строку по серьёзности, системе игры и происхождению, с фильтрами и поиском. Чужие кадры стек-трейсов свёрнуты.
- **Окно исключений** собирает все проблемы всех запусков, со счётчиками повторов и группировкой. Щелчок по кадру открывает нужную строку в твоём коде или в исходниках Minecraft и NeoForge.
  - У каждой проблемы есть владелец, то есть чья она, и тот, через кого она всплыла: сломанная текстура Steam 'n' Rails показывается как «Create: Steam 'n' Rails (railways) через Minecraft».
  - Моды показаны с названиями и логотипами. Плашка своего мода салатовая.
  - Сообщения размечены: id ресурсов окрашены цветом владельца, недопустимые символы в id подсвечены, классы и файлы выделены.
  - Объяснение простыми словами строится по самой глубокой причине в цепочке, а не по обёртке сверху.
- **Моды запуска** — всё, что попадает в игру: свой мод, Minecraft и загрузчик, чужие моды из Gradle и `run/mods`, моды внутри других jar и библиотеки.
  - У каждого мода видны логотип, версия, откуда он взялся, его зависимости и конфигурации миксинов.
  - Проблемы считаются по модам. Недостающая обязательная зависимость или мод, лежащий в запуске дважды, отмечаются ещё до того, как игра упадёт.

**Помогает писать код**
- **Редактор кода** с подсветкой и автосохранением.
- **Подсказки Java через jdtls:**
  - ошибки подчёркиваются прямо при наборе;
  - автодополнение само добавляет импорты;
  - при наведении показывается описание;
  - переход к определению (`Ctrl+щелчок`, `F12`, `Ctrl+B`), в том числе в код Minecraft.

  Языковой сервер — необязательный плагин: ставится одной кнопкой, когда ты этого хочешь.

**Удобен как дом**
- **Раскладка в духе Blender.** Любая область делится на вкладки. Их можно перетаскивать между областями и выносить в отдельные окна ОС.
- **Своя рамка окна.** Вместо заголовка Windows — верхняя строка программы с кнопками «Свернуть», «Развернуть» и «Закрыть». За строку окно перетаскивается, двойной щелчок разворачивает его.
- **Рабочие пространства** («Код», «Карта», «Ассеты», «Отладка»), у каждого своя раскладка. Её можно сохранить для проекта или как стандартную для всех.
- **Активность в Discord:** «Mod: Your Mod (yourmod)» с живым статусом (пишет код, запущен клиент, разбирается с крашем…), таймером и значком загрузчика. Её можно выключить, а проект — сделать секретным.
- **Интерфейс на русском и английском,** язык переключается на лету. Есть и **English · русские подсказки**: меню и термины на английском, как в руководствах по моддингу, а подсказки и объяснения по-русски. Другие языки добавляются файлами Fluent.
- **Темы, в которых перекрашивается всё:**
  - темы по умолчанию Mycelium Dark и Mycelium Light (в списках тем помечены «По умолчанию») и Visual Studio Dark — плагин темы, ставится одной кнопкой;
  - свои темы — файлы `.toml`, где задаётся только то, что меняется. Розовая шапка — буквально одна строка (`"menubar.bg" = "#ff69b4"`). Темы обновляются сразу после сохранения файла;
  - тема задаёт каждую область (шапку, строку состояния, вкладки, редактор, всплывающие окна, кнопки, консоль, цвета происхождения), подсветку кода с жирным и курсивом, а при желании — свои шрифты и фоновую картинку;
  - **быстрая смена темы** (``Ctrl+` ``): наведение или стрелки — посмотреть, Enter — оставить, Esc — вернуть прежнюю;
  - **значки** — из набора [Lucide](https://lucide.dev). Тема может заменить любой значок своим SVG и перекрасить любой значок ключом `icon.<имя>`;
  - **инструменты для авторов тем:** «Создать тему» пишет новый файл темы, где перечислены все ключи, классы подсветки и значки, а «Инспектор темы» показывает их все с текущими значениями.
- **Шрифты:** отдельные для интерфейса, редактора кода и консоли, из установленных в системе, с размером в пикселях и высотой строки. `Ctrl+колёсико` меняет размер шрифта редактора и консоли с шагом в полпикселя.
- **Фоновая картинка** полупрозрачно поверх окна, как в Rider.
- **Узкие области не ломаются.** Текст сокращается многоточием, панели переносятся или прячут фильтры в меню, лишние вкладки и рабочие пространства уходят в меню «»», а диалоги прокручиваются, а не вылезают за окно.
- **Плагины.** Всё большое и необязательное — плагин: сейчас языковой сервер Java и тема Visual Studio Dark, потом Kotlin и Python. На странице «Плагины» есть вкладки «Рекомендуемое» и «Установленные», установка в одну кнопку с прогрессом, включение и выключение, а наборы тем ставятся из `.zip`. Рекомендуемые плагины предлагаются и на экране приветствия.
- **Настройки в духе IDE от JetBrains:**
  - окно настроек с деревом страниц и поиском по всем пунктам;
  - изменения применяются кнопками «Применить» или «ОК» и отменяются «Отменой»;
  - раскладка клавиш, где любую команду можно переназначить, а занятые сочетания видны прямо при записи;
  - **«Найти действие»** (`Ctrl+Shift+A`);
  - экспорт и импорт настроек по категориям. Архив из другой программы Forma применяется в той части, которую программа понимает, остальное перечисляется в отчёте.

### План

1. ✅ **Каркас:** составной интерфейс, локализация, сборка.
2. ✅ **Проект и код:** `.mycelium`, модель Gradle, библиотеки по происхождению, запуск, умная консоль, окно исключений, jdtls.
3. 🔧 **Платформа Forma (идёт сейчас):**
   - ✅ настройки по группам как в Rider, экспорт и импорт;
   - ✅ раскладка клавиш и «Найти действие»;
   - ✅ темы, где перекрашивается всё, три независимых шрифта, фоновая картинка;
   - ✅ набор SVG-значков, которые темы могут заменять и перекрашивать, и инструменты для авторов тем;
   - ✅ плагины: менеджер плагинов, jdtls как необязательный плагин, наборы тем;
   - скрипты на Python и новые окна из плагинов.
4. 🔧 **Hyphae, движок знаний (в работе, см. [docs/hyphae.md](docs/hyphae.md#русский)).** Он понимает весь мод:
   - ✅ моды запуска и к какому моду относится каждый класс, ресурс и строка лога;
   - стороны клиента и сервера, потоки, этапы загрузки;
   - миксины и их цели;
   - что изменилось между версиями Minecraft и NeoForge и как переносить код.
5. **Карта мода:** граф твоих классов и ресурсов с автоматическими связями, группами как фреймы в Blender, стикерами и метками того, от чего зависит каждая часть.
6. **Ассеты:** превью JSON- и OBJ-моделей, проверка файлов данных, lang-файлы. Подсказки для Kotlin.

### На чём сделано

- [Rust](https://www.rust-lang.org/) и [egui](https://github.com/emilk/egui), отрисовка через [wgpu](https://wgpu.rs/) на Vulkan. Интерфейс программа рисует сама, без веб-движка.
- [Fluent](https://projectfluent.org/) для локализации.
- [Eclipse JDT Language Server](https://github.com/eclipse-jdtls/eclipse.jdt.ls) для подсказок Java.
- Значки [Lucide](https://lucide.dev) (лицензия ISC, см. `crates/forma-ui/icons/lucide/LICENSE`), рисуются с помощью [resvg](https://github.com/linebender/resvg).
- Init-скрипты Gradle для модели проекта. Никаких плагинов Gradle в мод не добавляется.

### Сборка

Нужны стабильный Rust (edition 2024) и драйвер видеокарты с поддержкой Vulkan. Разработка и проверка сейчас идут на Windows.

```bash
cargo run --release
```

Сразу открыть мод:

```bash
cargo run --release -- "D:\Моды\my-mod"
```

Собрать `dist\Forma Mycelium.exe`:

```bash
cargo xtask dist
```

Запустить тесты:

```bash
cargo test
```

### Горячие клавиши

Сочетания по умолчанию — как в IntelliJ IDEA. Любое можно поменять в **Настройках → Раскладка клавиш**.

| Действие | Сочетание |
|---|---|
| Найти действие | `Ctrl+Shift+A` |
| Быстрая смена темы | ``Ctrl+` `` |
| Настройки | `Ctrl+Alt+S` |
| Запустить / остановить | `Shift+F10` / `Ctrl+F2` |
| Синхронизировать с Gradle | `Ctrl+Shift+O` |
| Автодополнение | `Ctrl+Space` (после `.` открывается само) |
| Перейти к определению | `Ctrl+щелчок`, `F12` или `Ctrl+B` |
| Окно «Проект» / «Консоль» / «Исключения» | `Alt+1` / `Alt+4` / `Alt+6` |
| Развернуть область / вернуть | `Ctrl+Shift+F12` или двойной щелчок по вкладке |
| Закрыть вкладку | Средняя кнопка мыши по вкладке |
| Сохранить всё | `Ctrl+S` (файлы и так сохраняются сами) |
| Масштаб | `Ctrl+=` / `Ctrl+-` / `Ctrl+0` |

### Устройство репозитория

Крейты `forma-*` — платформа Forma, общая основа программ Forma. Крейты `mycelium-*` — всё, что относится к моддингу.

| Путь | Что внутри |
|---|---|
| `crates/mycelium` | Приложение: окно, меню, рабочие пространства, редакторы, запуск, клиент jdtls, Discord |
| `crates/mycelium-project` | Проект мода: `.mycelium`, распознавание, модель Gradle, библиотеки, происхождение кода |
| `crates/mycelium-log` | Разбор и классификация логов игры, Gradle и компиляторов |
| `crates/mycelium-hyphae` | Hyphae, движок знаний: jar, метаданные модов, вложенные jar, моды запуска и кому что принадлежит |
| `crates/forma-layout` | Составной интерфейс: дерево областей, вкладки, перетаскивание, окна ОС |
| `crates/forma-ui` | Движок тем, значки, шрифты, виджеты, окно настроек и его страницы, «Найти действие», инструменты для тем |
| `crates/forma-ui/icons/lucide/` | Значки Lucide, которые использует программа, и их лицензия |
| `crates/forma-plugins` | Плагины: описание, установка, удаление, включение и выключение |
| `crates/mycelium/themes/` | Встроенные темы |
| `crates/mycelium/plugins/` | Плагины, которые программа везёт в себе (тема Visual Studio Dark) |
| `crates/forma-actions` | Действия и раскладка клавиш |
| `crates/forma-settings` | Компоненты настроек, их файлы, экспорт и импорт |
| `crates/forma-i18n` | Локализация: Fluent, макрос `t!`, проверка полноты переводов |
| `locales/<язык>/*.ftl` | Переводы интерфейса (`platform.ftl` — строки платформы) |
| `assets/logo/` | Логотип: подробный для больших размеров, упрощённый для маленьких значков |
| `assets/discord/` | Картинки для активности в Discord (откуда они и лицензии — в README папки) |
| `xtask` | Служебные задачи сборки (`cargo xtask dist`, `cargo xtask discord-assets`) |

### Где хранятся данные

- `%APPDATA%\Forma\Mycelium\` — рабочие пространства, положение окна и стандартная раскладка. Настройки лежат в `options\`, по файлу на категорию (`appearance.json`, `fonts.json`, `keymap.json`…), а свои темы — в `themes\`. Переменная `FORMA_CONFIG_HOME` переносит их в другую папку, например для переносной копии (тогда туда же, в `cache\`, переезжают плагины и кеши).
- `%LOCALAPPDATA%\Forma\Mycelium\` — плагины (`plugins\`, например языковой сервер Java) и индексы по проектам. Метаданные Eclipse лежат здесь, а не в папке мода.
- `<мод>\.mycelium\` — сам проект:
  - `project.json` и `runs.json` предназначены для git;
  - `workspace.json` — твоя личная раскладка;
  - `cache\` — модель Gradle.

### О проекте

Forma Mycelium — часть FORMA. Идея и дизайн: Holo. Код полностью написан Claude (Anthropic).
