package cc.makin.urldebloater.android.settings

import cc.makin.urldebloater.android.rust.RedirectWashPolicyDiscriminants
import cc.makin.urldebloater.android.rust.TwitterPolicy
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
            redirectWashPolicy = RedirectWashPolicyDiscriminants.VIA_MIXER,
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
                SettingsCodec.USE_CLEAR_URLS to 1,
            )
        )

        assertEquals(AppSettings(), decoded)
    }
}
