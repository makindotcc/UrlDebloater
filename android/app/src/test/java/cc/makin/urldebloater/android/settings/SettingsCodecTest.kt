package cc.makin.urldebloater.android.settings

import org.junit.Assert.assertEquals
import org.junit.Test

class SettingsCodecTest {
    @Test
    fun emptyPreferencesDecodeToDefaults() {
        assertEquals(AppSettings(), SettingsCodec.decode(emptyMap()))
    }

    @Test
    fun nonDefaultsSurviveRoundTrip() {
        val settings = AppSettings(
            clipboardDebloater = false,
            redirectWashPolicy = RedirectWashPolicy.VIA_MIXER,
            mixerUrl = "https://example.com/mixer",
            useClearUrls = false,
            twitterPolicy = TwitterPolicy.FX_TWITTER,
        )

        assertEquals(settings, SettingsCodec.decode(SettingsCodec.encode(settings)))
    }

    @Test
    fun unknownEnumsAndInvalidUrlsFallBackToDefaults() {
        val decoded = SettingsCodec.decode(
            mapOf(
                SettingsCodec.REDIRECT_WASH_POLICY to "MIXER",
                SettingsCodec.TWITTER_POLICY to "THREADS",
                SettingsCodec.MIXER_URL to "not a url",
            )
        )

        assertEquals(AppSettings(), decoded)
    }

    @Test
    fun wrongTypesFallBackToDefaults() {
        val decoded = SettingsCodec.decode(
            mapOf(
                SettingsCodec.CLIPBOARD_DEBLOATER to "false",
                SettingsCodec.USE_CLEAR_URLS to 1,
            )
        )

        assertEquals(AppSettings(), decoded)
    }
}
