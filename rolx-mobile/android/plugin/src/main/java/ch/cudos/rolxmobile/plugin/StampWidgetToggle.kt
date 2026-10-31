package ch.cudos.rolxmobile.plugin

import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import android.widget.Toast
import org.json.JSONObject

/**
 * The widget's stamp button. Not exported, so only the widget's pending intent can stamp. Stamps in the background
 * (`goAsync`), as it reads and writes the stamp file; the Rust side also updates the notification.
 */
class StampWidgetToggle : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val app = context.applicationContext
        val pending = goAsync()
        Thread {
            try {
                toggle(app)
            } finally {
                pending.finish()
            }
        }.start()
    }

    private fun toggle(context: Context) {
        val result = try {
            JSONObject(RolxNative.toggle(context))
        } catch (e: Throwable) {
            Log.w(TAG, "stamping from the widget failed", e)
            JSONObject().put("result", "error").put("message", e.toString())
        }
        when (result.optString("result")) {
            "stale" -> toast(context, "Open RolX to end the forgotten session")
            "error" -> toast(context, "Could not stamp: ${result.optString("message")}")
        }
        if (result.optBoolean("upload")) {
            BackgroundUpload.schedule(context)
        }
        StampWidget.refreshAll(context)
    }

    private fun toast(context: Context, text: String) {
        android.os.Handler(android.os.Looper.getMainLooper()).post {
            Toast.makeText(context, text, Toast.LENGTH_LONG).show()
        }
    }

    companion object {
        private const val TAG = "RolxWidget"

        internal fun intent(context: Context): PendingIntent = PendingIntent.getBroadcast(
            context,
            0,
            Intent(context, StampWidgetToggle::class.java),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
    }
}
