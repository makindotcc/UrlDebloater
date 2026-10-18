package cc.makin.urldebloater.android

import cc.makin.urldebloater.android.rust.MobileUrlWasher
import cc.makin.urldebloater.android.rust.RedirectWashPolicyDiscriminants
import cc.makin.urldebloater.android.rust.createUrlWasher
import cc.makin.urldebloater.android.settings.AppSettings
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

class UrlWasherAdapter(
    private val clearurlsCachePath: String,
) : AutoCloseable {
    private var activeSettings: AppSettings? = null
    private var washer: MobileUrlWasher? = null

    suspend fun wash(text: String, settings: AppSettings): String = withContext(Dispatchers.IO) {
        val currentWasher = washer
        val activeWasher = if (currentWasher != null && activeSettings == settings) {
            currentWasher
        } else {
            val newWasher = createUrlWasher(
                redirectPolicy = settings.redirectWashPolicy,
                mixerUrl = settings.mixerUrl.trim().takeIf {
                    settings.redirectWashPolicy == RedirectWashPolicyDiscriminants.VIA_MIXER &&
                        it.isNotEmpty()
                },
                useClearUrls = settings.useClearUrls,
                clearurlsCachePath = clearurlsCachePath,
                twitterPolicy = settings.twitterPolicy,
            )
            currentWasher?.close()
            washer = newWasher
            activeSettings = settings
            newWasher
        }
        activeWasher.wash(text)
    }

    override fun close() {
        washer?.close()
        washer = null
        activeSettings = null
    }
}
