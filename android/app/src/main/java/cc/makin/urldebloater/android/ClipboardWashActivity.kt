package cc.makin.urldebloater.android

import android.content.ClipData
import android.content.ClipboardManager
import android.graphics.Color
import android.os.Bundle
import android.view.View
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.enableEdgeToEdge
import androidx.annotation.StringRes
import androidx.core.graphics.drawable.toDrawable
import androidx.lifecycle.lifecycleScope
import cc.makin.urldebloater.android.rust.UrlWasherInitException
import cc.makin.urldebloater.android.settings.SettingsStore
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import java.io.File

private const val clearurlsCacheFile = "clearurls-cache.json"
private const val clipboardTextMimeType = "text/*"

open class ClipboardWashActivity : ComponentActivity() {
    private var started = false
    private var adapter: UrlWasherAdapter? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        window.setBackgroundDrawable(Color.TRANSPARENT.toDrawable())
        setContentView(View(this))
        window.setLayout(1, 1)
        adapter = UrlWasherAdapter(File(filesDir, clearurlsCacheFile).absolutePath)
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (!hasFocus || started) return
        started = true
        washClipboard()
    }

    private fun washClipboard() {
        val clipboard = getSystemService(ClipboardManager::class.java)
        val text = try {
            clipboard.primaryClip
                ?.takeIf {
                    it.description.hasMimeType(clipboardTextMimeType) && it.itemCount > 0
                }
                ?.getItemAt(0)
                ?.coerceToText(this)
                ?.toString()
        } catch (_: Exception) {
            toastAndFinish(R.string.home_clipboard_error)
            return
        }
        if (text.isNullOrBlank()) {
            toastAndFinish(R.string.home_clipboard_empty_error)
            return
        }

        val settings = SettingsStore(this).load()
        val washer = checkNotNull(adapter)
        lifecycleScope.launch {
            val cleaned = try {
                washer.wash(text, settings).also { washed ->
                    clipboard.setPrimaryClip(
                        ClipData.newPlainText(getString(R.string.app_name), washed)
                    )
                }
            } catch (cancellation: CancellationException) {
                throw cancellation
            } catch (_: UrlWasherInitException) {
                toastAndFinish(R.string.settings_mixer_url_invalid)
                return@launch
            } catch (_: Exception) {
                toastAndFinish(R.string.home_clipboard_error)
                return@launch
            }
            onCleaned(cleaned)
        }
    }

    protected open fun onCleaned(cleaned: String) {
        Toast.makeText(this, R.string.home_clipboard_cleaned_message, Toast.LENGTH_SHORT).show()
        finish()
    }

    protected fun toastAndFinish(@StringRes messageRes: Int) {
        Toast.makeText(this, messageRes, Toast.LENGTH_SHORT).show()
        finish()
    }

    override fun onDestroy() {
        adapter?.close()
        adapter = null
        super.onDestroy()
    }
}
