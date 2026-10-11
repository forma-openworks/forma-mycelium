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

## About

about-version = Version { $version }
about-description = A Minecraft modding IDE that highlights and explains everything in Minecraft terms: exceptions, mixins, resources and the links inside your mod.
about-forma = Part of the FORMA project

## Messages

toast-project-missing = Project folder not found: { $path }
error-vulkan = Forma Mycelium could not start with Vulkan.

    Make sure your graphics driver supports Vulkan.

    Details: { $details }

## Project and layout

action-save-all = Save all
menu-run = Run
layout-save-default = Save layout as default
layout-save-default-hint = Every project that has no layout of its own will get this one
layout-apply-default = Apply the default layout
layout-reset-default = Reset the default layout to factory
status-problems = Open the exceptions window
toast-project-initialized = Project initialized: the .mycelium folder was created
toast-default-layout-saved = Layout saved as default
toast-default-layout-failed = Couldn't save the default layout
toast-default-layout-applied = Default layout applied
toast-default-layout-reset = Default layout reset to factory
toast-file-missing = File not found: { $path }
toast-source-missing = Source not found: { $class }
toast-no-sources = No sources for { $class } ({ $jar })
toast-sync-failed = Couldn't get the project model from Gradle: { $error }
status-syncing = Syncing with Gradle
toast-definition-missing = Definition not found
status-lsp-off = assistance off
status-lsp-error = jdtls error
status-lsp-busy = jdtls is reading the project
status-lsp-hint = Java assistance (jdtls): errors and warnings across the project

settings-discord = Show in Discord which mod I'm working on
settings-discord-secret = Secret project: hide this mod's name
settings-discord-secret-hint = Shows “a secret project” instead of the name, with no mod icon or repository link.

settings-page-discord = Discord
settings-discord-secret-no-project = Open a project to hide its name.
settings-component-appearance = Language, theme, accent color, text size, zoom
settings-component-keymap = Your changes to the shortcuts
settings-component-discord = Discord activity

menu-manage-settings = Manage settings
action-export-settings = Export settings…
action-import-settings = Import settings…
action-find-action = Find action…
action-sync-gradle = Sync with Gradle
action-run = Run the selected configuration
action-zoom-in = Zoom in
action-zoom-out = Zoom out
action-zoom-reset = Reset zoom
action-show-project = Project window
action-show-console = Console window
action-show-exceptions = Exceptions window
action-completion = Code completion
action-goto-definition = Go to definition

action-group-file = File
action-group-project = Project
action-group-run = Run
action-group-view = View
action-group-window = Window
action-group-editor = Code editor
action-group-help = Help
