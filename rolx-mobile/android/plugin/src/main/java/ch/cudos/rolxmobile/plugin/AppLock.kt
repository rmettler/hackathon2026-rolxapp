package ch.cudos.rolxmobile.plugin

import android.app.Activity
import android.app.KeyguardManager
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricManager.Authenticators.BIOMETRIC_WEAK
import androidx.biometric.BiometricManager.Authenticators.DEVICE_CREDENTIAL
import androidx.biometric.BiometricPrompt
import androidx.core.content.ContextCompat
import androidx.fragment.app.FragmentActivity

/**
 * The app lock's prompt: fingerprint, face or the phone's screen lock (PIN, pattern, password). Called from Rust
 * (`src/bridge.rs`), which polls `status` for the answer. It only guards the app's window: the widget and the
 * background upload keep working while the app is locked.
 */
object AppLock {
    const val PENDING = 0
    const val UNLOCKED = 1
    const val CANCELLED = 2

    /** Weak biometrics (most face unlocks) are enough: nothing is decrypted with the result. */
    private const val AUTHENTICATORS = BIOMETRIC_WEAK or DEVICE_CREDENTIAL

    @Volatile
    private var status = PENDING

    /** Whether the phone has a fingerprint, face or screen lock set up to unlock with. */
    @JvmStatic
    fun available(activity: Activity): Boolean {
        val result = BiometricManager.from(activity).canAuthenticate(AUTHENTICATORS)
        if (result == BiometricManager.BIOMETRIC_SUCCESS) {
            return true
        }
        val keyguard = activity.getSystemService(KeyguardManager::class.java)
        return keyguard?.isDeviceSecure == true
    }

    /** Shows the prompt; `status` turns from `PENDING` to `UNLOCKED` or `CANCELLED`. */
    @JvmStatic
    fun authenticate(activity: Activity, title: String, subtitle: String) {
        status = PENDING
        val host = activity as? FragmentActivity
        if (host == null) {
            status = CANCELLED
            return
        }
        host.runOnUiThread {
            try {
                val prompt = BiometricPrompt(host, ContextCompat.getMainExecutor(host), callback)
                val info = BiometricPrompt.PromptInfo.Builder()
                    .setTitle(title)
                    .setSubtitle(subtitle)
                    .setAllowedAuthenticators(AUTHENTICATORS)
                    .build()
                prompt.authenticate(info)
            } catch (e: Exception) {
                android.util.Log.w("RolxLock", "cannot show the unlock prompt", e)
                status = CANCELLED
            }
        }
    }

    @JvmStatic
    fun status(): Int = status

    private val callback = object : BiometricPrompt.AuthenticationCallback() {
        override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
            status = UNLOCKED
        }

        override fun onAuthenticationError(errorCode: Int, errString: CharSequence) {
            status = CANCELLED
        }

        // A finger that was not recognised: the prompt stays open for another try.
        override fun onAuthenticationFailed() {}
    }
}
