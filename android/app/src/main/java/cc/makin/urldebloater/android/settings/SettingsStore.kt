package cc.makin.urldebloater.android.settings

import android.content.Context
import android.content.SharedPreferences
import kotlin.enums.enumEntries
import androidx.core.content.edit

internal object SettingsCodec {
    const val CLIPBOARD_DEBLOATER = "clipboard_debloater"
    const val REDIRECT_WASH_POLICY = "redirect_wash_policy"
    const val MIXER_URL = "mixer_url"
    const val USE_CLEAR_URLS = "use_clear_urls"
    const val TWITTER_POLICY = "twitter_policy"

    fun decode(values: Map<String, Any?>): AppSettings {
        val defaults = AppSettings()
        return AppSettings(
            clipboardDebloater = values[CLIPBOARD_DEBLOATER] as? Boolean
                ?: defaults.clipboardDebloater,
            redirectWashPolicy = enumValue(
                values[REDIRECT_WASH_POLICY],
                defaults.redirectWashPolicy
            ),
            mixerUrl = (values[MIXER_URL] as? String)?.trim()?.takeIf(::isValidMixerUrl)
                ?: defaults.mixerUrl,
            useClearUrls = values[USE_CLEAR_URLS] as? Boolean ?: defaults.useClearUrls,
            twitterPolicy = enumValue(values[TWITTER_POLICY], defaults.twitterPolicy),
        )
    }

    fun encode(settings: AppSettings): Map<String, Any> = mapOf(
        CLIPBOARD_DEBLOATER to settings.clipboardDebloater,
        REDIRECT_WASH_POLICY to settings.redirectWashPolicy.name,
        MIXER_URL to settings.mixerUrl,
        USE_CLEAR_URLS to settings.useClearUrls,
        TWITTER_POLICY to settings.twitterPolicy.name,
    )

    private inline fun <reified T : Enum<T>> enumValue(raw: Any?, default: T): T {
        val name = raw as? String ?: return default
        return enumEntries<T>().firstOrNull { it.name == name } ?: default
    }
}

class SettingsStore(context: Context) {
    private val preferences: SharedPreferences =
        context.getSharedPreferences("settings", Context.MODE_PRIVATE)

    fun load(): AppSettings = SettingsCodec.decode(preferences.all)

    fun save(settings: AppSettings) {
        preferences.edit {
            for ((key, value) in SettingsCodec.encode(settings)) {
                when (value) {
                    is Boolean -> putBoolean(key, value)
                    is String -> putString(key, value)
                    else -> error("Unsupported preference type for $key")
                }
            }
        }
    }
}
