# Running, console and the exceptions window.

run-client = Client
run-server = Server
run-data = Data generation
run-build = Build
run-other = Task
run-clean = Clean
run-no-configs = No run configurations
run-start = Run
run-stop = Stop
run-restart = Restart
run-seconds = { $count } s
run-status-running = { $name } · running { $time }
run-status-done = { $name } · done in { $time }
run-status-failed = { $name } · failed (code { $code }) after { $time }
run-status-stopped = { $name } · stopped after { $time }
run-status-error = Failed to start: { $error }

console-no-project-title = No project
console-no-project-hint = Open a mod project to run the client, the server and builds.
console-empty-title = All quiet
console-empty-hint = Run the client with ▶ (Shift+F10) — its log will appear here, sorted by game system and origin.
console-filtered-title = Nothing matches
console-filtered-hint = No entries match the selected filters.
console-clear = Clear console
console-autoscroll = Scroll to new entries
console-only-own = Only my mod
console-only-problems = Only problems
console-level-all = All levels
console-level-info = INFO and above
console-level-warn = Warnings and errors
console-level-error = Errors only
console-search = Search
console-filters = Filters
console-show-in-exceptions = Show in the exceptions window

exceptions-by-system = System
exceptions-by-type = Type
exceptions-by-origin = Origin
exceptions-by-severity = Severity
exceptions-only-errors = Errors only
exceptions-clear = Clear the list
exceptions-empty-title = No problems
exceptions-empty-hint = Run the client — every exception, error and warning will land here, grouped by game system.
exceptions-pick = Pick a problem to see the details and the explanation.
exceptions-new = new
exceptions-new-in-run = first seen in this run
exceptions-not-in-last-run = didn't happen in the last run
exceptions-count = { $count ->
    [one] { $count } time
   *[other] { $count } times
}
exceptions-severity-crash = Crashes
exceptions-severity-error = Errors
exceptions-severity-warning = Warnings
exceptions-type-error = Errors in the log
exceptions-type-warning = Warnings in the log
exceptions-noise-group = Harmless noise
exceptions-noise-hint = Warnings the game, the loader and other mods print on every run in a development environment. They have nothing to do with your mod.
exceptions-only-noise = No real problems — only harmless noise.

noise-tag = noise
noise-title = Harmless noise
noise-explain = Minecraft, the loader or another mod prints this warning on every run in a development environment, with any mod. There is nothing to fix in your code — it's safe to skip.

frame-open-library-source = Open source (read-only)
frame-open-source = Open source
frames-hidden = { $count ->
    [one] { $count } frame
   *[other] { $count } frames
}
frames-omitted = { $count ->
    [one] { $count } more frame is the same as in the exception above
   *[other] { $count } more frames are the same as in the exception above
}

origin-own = My mod
origin-mod = Another mod
origin-mod-named = Mod { $id }
origin-library = Library
origin-unknown = Unknown

system-mod-loading = Mod loading
system-registries = Registries
system-resources = Resources
system-data = Data
system-mixin = Mixins
system-network = Network
system-rendering = Rendering
system-sound = Sound
system-world = World
system-config = Configs
system-build = Build
system-game = Game

run-edit-configs = Edit configurations…
run-close-tab = Close the tab
run-stop-all = Stop all
console-waiting-title = Starting
console-waiting-hint = Gradle is getting ready — the log shows up as soon as the task starts writing.

runcfg-title = Run configurations
runcfg-new = New
runcfg-new-name = New configuration
runcfg-duplicate = Duplicate
runcfg-copy-name = { $name } (copy)
runcfg-hide = Hide from the run list
runcfg-show = Bring back to the run list
runcfg-delete = Delete
runcfg-hidden = hidden
runcfg-empty = No configurations — add your own with “New”.
runcfg-detected-note = Found in the project. Edits are stored on top of it, and you can always restore the original.
runcfg-custom-note = Your own configuration.
runcfg-name = Name
runcfg-kind = Kind
runcfg-kind-hint = The kind sets the icon and the Discord status.
runcfg-task = Gradle tasks
runcfg-args = Gradle arguments
runcfg-reset = Restore the original
runcfg-saved-in = Stored in .mycelium/runs.json — fine to commit to git.

problem-via = via
problem-owner-hint = Whose problem it is: who should fix it
problem-via-hint = Who noticed or reported it
