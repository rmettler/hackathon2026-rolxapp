package ch.cudos.rolxmobile.plugin

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.os.SystemClock
import android.util.Log
import android.view.View
import android.widget.RemoteViews
import org.json.JSONObject

/**
 * The home-screen widget: the running session with a timer, and a button that stamps in or out like the desktop
 * hotkey (`StampWidgetToggle`). Its content comes from the Rust library (`RolxNative.widgetState`); the app refreshes
 * it whenever the stamps change (`refreshAll`, called from `src/bridge.rs`).
 */
class StampWidget : AppWidgetProvider() {
    override fun onUpdate(context: Context, manager: AppWidgetManager, ids: IntArray) {
        render(context, manager, ids)
    }

    companion object {
        private const val TAG = "RolxWidget"

        /** Redraws every widget on the home screens, e.g. after the stamps changed. */
        @JvmStatic
        fun refreshAll(context: Context) {
            val manager = AppWidgetManager.getInstance(context) ?: return
            val ids = manager.getAppWidgetIds(ComponentName(context, StampWidget::class.java))
            if (ids.isNotEmpty()) {
                render(context, manager, ids)
            }
        }

        private fun render(context: Context, manager: AppWidgetManager, ids: IntArray) {
            val state = try {
                JSONObject(RolxNative.widgetState(context))
            } catch (e: Throwable) {
                Log.w(TAG, "cannot read the stamp state", e)
                null
            }
            val views = views(context, state)
            for (id in ids) {
                manager.updateAppWidget(id, views)
            }
        }

        private fun views(context: Context, state: JSONObject?): RemoteViews {
            val views = RemoteViews(context.packageName, R.layout.widget_stamp)
            val openApp = openAppIntent(context)
            views.setOnClickPendingIntent(R.id.widget_root, openApp)

            if (state == null || state.has("error")) {
                views.setTextViewText(R.id.widget_title, "RolX")
                showHeadline(views, "Open RolX")
                views.setTextViewText(R.id.widget_text, state?.optString("error") ?: "")
                views.setViewVisibility(R.id.widget_toggle, View.GONE)
                return views
            }

            views.setTextViewText(R.id.widget_title, state.getString("title"))
            views.setTextViewText(R.id.widget_text, state.getString("text"))
            if (state.getBoolean("timer")) {
                // The chronometer counts from a base in the boot clock, so it keeps running without the app.
                val base = SystemClock.elapsedRealtime() - state.getLong("workedMs")
                views.setChronometer(R.id.widget_timer, base, null, state.getBoolean("running"))
                views.setViewVisibility(R.id.widget_timer, View.VISIBLE)
                views.setViewVisibility(R.id.widget_headline, View.GONE)
            } else {
                showHeadline(views, state.getString("headline"))
            }

            val stampedIn = state.getBoolean("stampedIn")
            views.setImageViewResource(
                R.id.widget_toggle,
                if (stampedIn) R.drawable.ic_widget_stop else R.drawable.ic_widget_play,
            )
            views.setContentDescription(
                R.id.widget_toggle,
                context.getString(if (stampedIn) R.string.widget_stamp_out else R.string.widget_stamp_in),
            )
            // A forgotten session must not be closed silently at "now": the button opens the app instead, which
            // asks when the work ended.
            val action = if (state.getBoolean("stale")) openApp else StampWidgetToggle.intent(context)
            views.setOnClickPendingIntent(R.id.widget_toggle, action)
            views.setViewVisibility(R.id.widget_toggle, View.VISIBLE)
            return views
        }

        private fun showHeadline(views: RemoteViews, text: String) {
            views.setTextViewText(R.id.widget_headline, text)
            views.setViewVisibility(R.id.widget_headline, View.VISIBLE)
            views.setViewVisibility(R.id.widget_timer, View.GONE)
        }

        /** Brings the app to the front, as its launcher icon does. */
        internal fun openAppIntent(context: Context): PendingIntent {
            val launch = context.packageManager.getLaunchIntentForPackage(context.packageName)
                ?: Intent().setPackage(context.packageName)
            return PendingIntent.getActivity(
                context,
                0,
                launch,
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            )
        }
    }
}
