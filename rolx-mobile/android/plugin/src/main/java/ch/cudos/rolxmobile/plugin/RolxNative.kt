package ch.cudos.rolxmobile.plugin

import android.content.Context

/**
 * The Rust library's functions for the parts that run without the app's window: the home-screen widget and the
 * background upload, see `src/bridge.rs`. They may run in a process the app was never started in, so this loads the
 * library itself (once per process; the app's activity loads the same one).
 *
 * Each call returns a JSON object (described at the Rust function) and never throws for an error in Rust.
 */
object RolxNative {
    init {
        System.loadLibrary("main")
    }

    /** What the widget shows: `title`, `headline` or a timer (`timer`, `running`, `workedMs`), `text`, `stale`. */
    @JvmStatic
    external fun widgetState(context: Context): String

    /** Stamps in or out like the desktop hotkey: `result` is `in`, `out`, `stale` (refused) or `error`. */
    @JvmStatic
    external fun toggle(context: Context): String

    /** Uploads the queue: `result` is `done`, `retry` or `login`; `uploaded` and `waiting` count entries. */
    @JvmStatic
    external fun upload(context: Context): String
}
