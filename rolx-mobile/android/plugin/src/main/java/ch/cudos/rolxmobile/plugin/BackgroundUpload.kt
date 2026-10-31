package ch.cudos.rolxmobile.plugin

import android.content.Context
import android.util.Log
import androidx.work.BackoffPolicy
import androidx.work.Constraints
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequest
import androidx.work.PeriodicWorkRequest
import androidx.work.WorkManager
import androidx.work.Worker
import androidx.work.WorkerParameters
import java.util.concurrent.TimeUnit
import org.json.JSONObject

/**
 * Uploads the stamped entries while the app is closed, with WorkManager: as soon as there is a network after the
 * stamps changed (`schedule`), and every hour as a safety net (`start`). Android decides the exact time (battery
 * saving); the app still uploads right away while it is open. Called from `src/bridge.rs` and the widget.
 */
object BackgroundUpload {
    private const val NOW = "rolx-upload"
    private const val PERIODIC = "rolx-upload-periodic"
    private const val PERIOD_HOURS = 1L
    private const val BACKOFF_SECONDS = 30L

    private val online = Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build()

    /** Uploads once a network is there. Appended to an upload that is still running or waiting, so stamps made
     *  meanwhile are not left out. */
    @JvmStatic
    fun schedule(context: Context) {
        val request = OneTimeWorkRequest.Builder(UploadWorker::class.java)
            .setConstraints(online)
            .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, BACKOFF_SECONDS, TimeUnit.SECONDS)
            .build()
        WorkManager.getInstance(context)
            .enqueueUniqueWork(NOW, ExistingWorkPolicy.APPEND_OR_REPLACE, request)
    }

    /** Sets up the hourly upload, unless it is already set up. Called at every app start. */
    @JvmStatic
    fun start(context: Context) {
        val request = PeriodicWorkRequest.Builder(UploadWorker::class.java, PERIOD_HOURS, TimeUnit.HOURS)
            .setConstraints(online)
            .build()
        WorkManager.getInstance(context)
            .enqueueUniquePeriodicWork(PERIODIC, ExistingPeriodicWorkPolicy.KEEP, request)
    }
}

/** One upload of the queue by the Rust library (`RolxNative.upload`). */
class UploadWorker(context: Context, params: WorkerParameters) : Worker(context, params) {
    override fun doWork(): Result {
        val context = applicationContext
        val report = try {
            JSONObject(RolxNative.upload(context))
        } catch (e: Throwable) {
            Log.w(TAG, "background upload failed", e)
            return retryOrGiveUp()
        }
        val waiting = report.optInt("waiting")
        return when (report.optString("result")) {
            // Retrying won't help until the user logs in again; the app or the next stamp schedules a new upload.
            "login" -> {
                LoginReminder.show(context, waiting)
                Result.success()
            }
            "retry" -> {
                Log.i(TAG, "upload incomplete: ${report.optString("message")}")
                retryOrGiveUp()
            }
            else -> {
                LoginReminder.cancel(context)
                if (report.optInt("uploaded") > 0) {
                    StampWidget.refreshAll(context)
                }
                Result.success()
            }
        }
    }

    /** A day the server keeps rejecting (e.g. before the edit lock) must not retry forever; the hourly upload and
     *  the app still try again later. */
    private fun retryOrGiveUp(): Result =
        if (runAttemptCount < MAX_ATTEMPTS) Result.retry() else Result.failure()

    private companion object {
        const val TAG = "RolxUpload"
        const val MAX_ATTEMPTS = 5
    }
}
