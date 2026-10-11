# Known problems: a short title and a plain-language explanation.

known-missing-texture-title = Texture not found
known-missing-texture-explain = A model refers to a texture that isn't in the resources — the game shows a purple-and-black checkerboard instead. Make sure the file is at assets/<mod>/textures/<path>.png and the model refers to it without "textures/" and ".png", in lowercase, e.g. "mymod:item/ruby".

known-missing-model-title = Model not found
known-missing-model-explain = The game couldn't find the JSON model of a block or item. Usually the file is missing at assets/<mod>/models/item/<name>.json (or models/block/), or there's a typo in the blockstate or item model. The model name must match the id the object is registered under.

known-blockstate-title = Blockstate error
known-blockstate-explain = The block state file (assets/<mod>/blockstates/<name>.json) didn't load or doesn't cover every state of the block. Check the JSON and that every variant (e.g. facing=north) points to an existing model.

known-invalid-id-title = Invalid id
known-invalid-id-explain = A resource or registry id contains characters that aren't allowed. Minecraft ids use only lowercase Latin letters, digits and _ - . / — for example "ruby_block", not "RubyBlock".

known-client-on-server-title = Client class on the server
known-client-on-server-explain = Code that also runs on the server touched a class that exists only on the client (usually from net.minecraft.client). A dedicated server has no such classes, so the game crashes. Move client code into a separate class that is loaded only on the client (NeoForge: Dist.CLIENT, Fabric: ClientModInitializer).

known-mixin-title = Mixin failed to apply
known-mixin-explain = A mixin couldn't be woven into its target: a method or field wasn't found, the signature didn't match, or the target changed in this game version. Check the method name and descriptor in @Inject and @At and that the mixin is listed in the mod's mixin config.

known-registry-unbound-title = Registry object not registered yet
known-registry-unbound-explain = Code accessed an object from a DeferredRegister before registration happened, or the register isn't attached to the mod event bus. Make sure register(modEventBus) is called and don't touch registry objects in static fields or constructors before registration.

known-class-missing-title = Class or method not found
known-class-missing-explain = A class, method or field that existed at compile time is missing at runtime. Usually the mod was built for a different version of the game, the loader or a dependency, or a required mod isn't installed.

known-null-pointer-title = NullPointerException
known-null-pointer-explain = Code used an object that isn't there (null). Look at the first line of your mod in the stack trace — a variable or a call result was empty there. A common cause in modding is touching the world, a player or a registry object before it exists.

known-data-parse-title = Data error
known-data-parse-explain = A file from data/ (recipe, loot table, tag, advancement) couldn't be parsed. Usually it's a JSON mistake or an unknown item or recipe type id. Open the file from the message and check commas, brackets and ids.

known-missing-dependency-title = Missing dependency
known-missing-dependency-explain = The mod needs another mod (or another loader or game version) that isn't installed or doesn't match. Add the dependency to build.gradle or fix the version range in the mod metadata.

known-duplicate-mod-title = Mod loaded twice
known-duplicate-mod-explain = The same mod was found twice — for example in the mods folder and as a Gradle dependency. Remove the extra copy.

known-mod-construction-title = Mod failed to load
known-mod-construction-explain = The loader couldn't construct the mod or finish one of its loading stages. The real reason is usually further down the stack trace (Caused by) — look for the first line of your mod.

known-out-of-memory-title = Out of memory
known-out-of-memory-explain = Java ran out of memory. Give the run more memory (e.g. -Xmx4G) or check whether the mod keeps accumulating data.

known-concurrent-modification-title = Collection changed during iteration
known-concurrent-modification-explain = A list or map was modified while a loop was walking over it, or from another thread. In Minecraft this often happens when the world is touched outside the main server thread.

known-compile-error-title = Compilation error
known-compile-error-explain = The code didn't compile. Click the error location to open the file at that line.

known-build-failed-title = Gradle build failed
known-build-failed-explain = Gradle stopped the build. The details say what went wrong: most often a compilation error or a dependency that couldn't be downloaded.
