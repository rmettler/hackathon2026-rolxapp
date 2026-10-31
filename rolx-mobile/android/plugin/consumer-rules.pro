# Applied to the app's release build, which shrinks and renames code. Rust calls into this module through JNI by
# class and method name (src/bridge.rs, src/android.rs) and implements `RolxNative`'s native methods, which R8 cannot
# see, so the whole (small) module is kept as is.
-keep class ch.cudos.rolxmobile.plugin.** { *; }
