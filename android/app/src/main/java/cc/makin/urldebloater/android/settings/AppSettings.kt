package cc.makin.urldebloater.android.settings

import androidx.annotation.StringRes
import cc.makin.urldebloater.android.R
import java.net.URI

const val PUBLIC_MIXER_INSTANCE = "https://urldebloater.makin.cc/"

data class AppSettings(
    val clipboardDebloater: Boolean = true,
    val redirectWashPolicy: RedirectWashPolicy = RedirectWashPolicy.LOCALLY,
    val mixerUrl: String = PUBLIC_MIXER_INSTANCE,
    val useClearUrls: Boolean = true,
    val twitterPolicy: TwitterPolicy = TwitterPolicy.ORIGINAL,
)

enum class RedirectWashPolicy(@param:StringRes val labelRes: Int) {
    IGNORE(R.string.settings_policy_ignore),
    LOCALLY(R.string.settings_policy_locally),
    VIA_MIXER(R.string.settings_policy_via_mixer),
}

enum class TwitterPolicy(@param:StringRes val labelRes: Int) {
    ORIGINAL(R.string.settings_twitter_original),
    VX_TWITTER(R.string.settings_twitter_vx),
    FX_TWITTER(R.string.settings_twitter_fx),
}

internal fun isValidMixerUrl(raw: String): Boolean {
    val uri = runCatching { URI(raw.trim()) }.getOrNull() ?: return false
    val scheme = uri.scheme ?: return false
    val isHttp = scheme.equals("http", ignoreCase = true) ||
        scheme.equals("https", ignoreCase = true)
    return isHttp && !uri.host.isNullOrBlank()
}
