package cc.makin.urldebloater.android.settings

import androidx.annotation.StringRes
import cc.makin.urldebloater.android.R
import cc.makin.urldebloater.android.rust.RedirectWashPolicyDiscriminants
import cc.makin.urldebloater.android.rust.TwitterPolicy
import java.net.URI

const val PUBLIC_MIXER_INSTANCE = "https://urldebloater.makin.cc/"

data class AppSettings(
    val clipboardDebloater: Boolean = true,
    val redirectWashPolicy: RedirectWashPolicyDiscriminants = RedirectWashPolicyDiscriminants.LOCALLY,
    val mixerUrl: String = PUBLIC_MIXER_INSTANCE,
    val useClearUrls: Boolean = true,
    val twitterPolicy: TwitterPolicy = TwitterPolicy.ORIGINAL,
)

@get:StringRes
val RedirectWashPolicyDiscriminants.labelRes: Int
    get() = when (this) {
        RedirectWashPolicyDiscriminants.IGNORE -> R.string.settings_policy_ignore
        RedirectWashPolicyDiscriminants.LOCALLY -> R.string.settings_policy_locally
        RedirectWashPolicyDiscriminants.VIA_MIXER -> R.string.settings_policy_via_mixer
    }

@get:StringRes
val TwitterPolicy.labelRes: Int
    get() = when (this) {
        TwitterPolicy.ORIGINAL -> R.string.settings_twitter_original
        TwitterPolicy.VX_TWITTER -> R.string.settings_twitter_vx
        TwitterPolicy.FX_TWITTER -> R.string.settings_twitter_fx
    }

internal fun isValidMixerUrl(raw: String): Boolean {
    val uri = runCatching { URI(raw.trim()) }.getOrNull() ?: return false
    val scheme = uri.scheme ?: return false
    val isHttp = scheme.equals("http", ignoreCase = true) ||
        scheme.equals("https", ignoreCase = true)
    return isHttp && !uri.host.isNullOrBlank()
}
