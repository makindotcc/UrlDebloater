package cc.makin.urldebloater.android

import android.app.Application
import android.content.Context
import android.content.pm.ApplicationInfo
import cc.makin.urldebloater.android.settings.AppSettings
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import java.io.File

internal const val clearurlsCacheFile = "clearurls-cache.json"

class UrlDebloaterApplication : Application() {
    private val washerMutex = Mutex()
    private var washer: UrlWasherAdapter? = null

    override fun onCreate() {
        super.onCreate()
        val debuggable = applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE != 0
        NativeBridge.initLogger(debuggable)
        NativeBridge.initTls(applicationContext)
        publishClipboardShortcut(this)
    }

    private suspend fun <T> withWasher(block: suspend (UrlWasherAdapter) -> T): T =
        washerMutex.withLock {
            val activeWasher = washer ?: UrlWasherAdapter(
                File(filesDir, clearurlsCacheFile).absolutePath
            ).also { washer = it }
            block(activeWasher)
        }

    internal suspend fun washOutcome(
        text: String,
        settings: AppSettings,
    ): WashOutcome = withWasher { it.washOutcome(text, settings) }

    internal suspend fun washClipboard(
        context: Context,
        text: String,
        settings: AppSettings,
    ): String? = withWasher { context.washClipboardText(it, text, settings) }

}
