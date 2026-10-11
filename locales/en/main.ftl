# The language's own name — shown in the settings.
language-name = English

## Menus

menu-file = File
menu-view = View
menu-window = Window
menu-help = Help
menu-recent-projects = Recent projects
menu-recent-empty = Nothing yet
menu-open-editor = Open editor
menu-new-workspace = New workspace

## Actions

action-open-project = Open mod project…
action-close-project = Close project
action-settings = Settings…
action-quit = Quit
action-about = About
action-return-windows = Return all detached windows
action-reset-workspace = Reset workspace layout
action-reset-all-workspaces = Reset all workspaces
action-done = Done
action-close = Close
action-rename = Rename
action-cancel = Cancel

dialog-open-project = Mod project folder

## Workspaces

workspace-code = Code
workspace-map = Map
workspace-assets = Assets
workspace-debug = Debug
workspace-untitled = Workspace
workspace-copy-name = { $name } (copy)
workspace-new-copy = Copy of current
workspace-rename = Rename…
workspace-rename-title = Workspace name
workspace-duplicate = Duplicate
workspace-delete = Delete

## Status bar

status-no-project = No project open
status-renderer = Renderer
status-detached-windows = { $count ->
    [one] { $count } detached window
   *[other] { $count } detached windows
}

## Welcome screen

welcome-tagline = A Minecraft modding IDE for Fabric and NeoForge, Java and Kotlin. It understands Minecraft, not just Java.
welcome-continue = Continue without a project
welcome-recent = Recent projects

## Settings

settings-title = Settings
settings-language = Interface language
settings-language-system = System default
settings-theme = Theme
settings-accent = Accent color
settings-font-size = Text size
settings-locales-hint = You can add your own translations to { $path } (a language folder with .ftl files) — they are picked up on the next launch.
theme-dark = Dark
theme-light = Light
view-zoom-hint = Zoom: Ctrl+= / Ctrl+- / Ctrl+0

accent-violet = Violet
accent-blue = Blue
accent-teal = Teal
accent-green = Green
accent-amber = Amber
accent-rose = Rose

## About

about-version = Version { $version }
about-description = A Minecraft modding IDE that highlights and explains everything in Minecraft terms: exceptions, mixins, resources and the links inside your mod.
about-forma = Part of the FORMA project

## Messages

toast-project-missing = Project folder not found: { $path }
error-vulkan = Forma Mycelium could not start with Vulkan.

    Make sure your graphics driver supports Vulkan.

    Details: { $details }
