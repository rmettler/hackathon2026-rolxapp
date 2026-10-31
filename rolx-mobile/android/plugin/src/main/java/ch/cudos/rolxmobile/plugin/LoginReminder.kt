package ch.cudos.rolxmobile.plugin

import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat

/**
 * A quiet notification when the background upload cannot send the stamped entries because the login has expired.
 * Tapping it opens the app; it disappears after the next successful upload. Not shown without the notification
 * permission (asked for at the first stamp-in, see `src/native.rs`).
 */
object LoginReminder {
    /** The stamp notification (`src/android.rs`) uses id 1. */
    private const val ID = 2
    private const val CHANNEL = "upload"

    fun show(context: Context, waiting: Int) {
        if (waiting <= 0) {
            cancel(context)
            return
        }
        val manager = NotificationManagerCompat.from(context)
        if (!manager.areNotificationsEnabled()) {
            return
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(CHANNEL, "Uploads", NotificationManager.IMPORTANCE_LOW)
            channel.description = "Tells you when stamped entries wait for a login to be uploaded"
            context.getSystemService(NotificationManager::class.java)?.createNotificationChannel(channel)
        }
        val entries = if (waiting == 1) "1 entry" else "$waiting entries"
        val notification = NotificationCompat.Builder(context, CHANNEL)
            .setSmallIcon(R.drawable.ic_rolx_notification)
            .setContentTitle("Log in to upload $entries")
            .setContentText("Your RolX login has expired. The entries are safe on the phone.")
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setContentIntent(StampWidget.openAppIntent(context))
            .setAutoCancel(true)
            .build()
        try {
            manager.notify(ID, notification)
        } catch (e: SecurityException) {
            // The permission was revoked between the check and the call.
        }
    }

    fun cancel(context: Context) {
        NotificationManagerCompat.from(context).cancel(ID)
    }
}
