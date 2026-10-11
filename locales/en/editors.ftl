# Editors shown in tabs.

editor-project = Project
editor-code = Code
editor-mod-map = Mod map
editor-exceptions = Exceptions
editor-console = Console
editor-model-preview = Model preview

editor-project-about = Files of the mod project.
editor-code-about = A Java and Kotlin editor with jdtls and kotlin-lsp completion, Minecraft-aware checks, and mixins that make sense at a glance.
editor-mod-map-about = A graph of the mod's classes and resources: links appear on their own, nodes can be grouped like Blender frames and annotated with sticky notes. A ring around each node shows what it depends on: Minecraft, NeoForge, Fabric or other mods.
editor-exceptions-about = Every exception and error, grouped by type and game system, with history across runs and plain-language explanations.
editor-console-about = Run the client and server, with a log sorted by severity, game system and origin.
editor-model-preview-about = 3D preview of block and item models: Minecraft JSON models and OBJ, block states, display modes and textures.
editor-coming-stage = Coming in stage { $stage }

project-none-title = No project open
project-none-hint = Open the folder of a Gradle mod project — Fabric or NeoForge.
project-refresh = Reload files

code-empty-title = No file open
code-empty-hint = Double-click a file in the project tree or click a line of your mod in a stack trace.
code-cannot-open = The file can't be opened as text
code-binary = This is a binary file.
code-not-utf8 = The file isn't UTF-8.
code-too-large = The file is too large for the editor.

project-collapse-all = Collapse all
project-show-hidden = Show hidden files and folders
project-hide-hidden = Hide hidden files and folders
project-not-a-mod = No loader found — this doesn't look like a mod
project-no-gradle = No build.gradle — this isn't a Gradle project
project-open = Open
project-reveal = Show in Explorer
project-copy-path = Copy path
project-copy-relative = Copy path from project root
code-readonly = read-only

libs-title = External libraries
libs-syncing = syncing with Gradle…
libs-indexing = indexing libraries…
libs-sync-failed = Gradle didn't answer
libs-sync-failed-hint = Check build.gradle and the network, then pick "Sync with Gradle" in this row's menu. The full output is in the app log.
libs-sync = Sync with Gradle
libs-mods = Other mods
libs-libraries = Libraries
libs-mojang = Mojang libraries
libs-loader-parts = Loader components
libs-has-sources = Sources available — classes open as code
libs-no-sources = No sources
libs-copy-class = Copy class name
libs-reveal-jar = Show jar in Explorer

code-completion-loading = Looking for options…
lsp-not-installed = Java assistance is off: jdtls is needed (about 50 MB).
lsp-install = Install
lsp-downloading-of = Downloading jdtls: { $done } of { $total } MB
lsp-downloading = Downloading jdtls: { $done } MB
lsp-extracting = Unpacking jdtls…
lsp-no-jdk = Java assistance needs JDK { $version } or newer, and none was found.
lsp-starting = Starting jdtls…
lsp-importing = jdtls is reading the project
lsp-failed = jdtls is not working: { $error }
lsp-restart = Restart
