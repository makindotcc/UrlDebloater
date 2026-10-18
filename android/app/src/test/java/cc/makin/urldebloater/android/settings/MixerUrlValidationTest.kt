package cc.makin.urldebloater.android.settings

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class MixerUrlValidationTest {
    @Test
    fun acceptsHttpUrls() {
        assertTrue(isValidMixerUrl(PUBLIC_MIXER_INSTANCE))
        assertTrue(isValidMixerUrl("https://example.com/mixer"))
        assertTrue(isValidMixerUrl("http://example.com"))
        assertTrue(isValidMixerUrl("  https://example.com  "))
    }

    @Test
    fun rejectsBlankAndNonHttpUrls() {
        assertFalse(isValidMixerUrl(""))
        assertFalse(isValidMixerUrl("   "))
        assertFalse(isValidMixerUrl("example.com"))
        assertFalse(isValidMixerUrl("ftp://example.com"))
        assertFalse(isValidMixerUrl("https://"))
        assertFalse(isValidMixerUrl("not a url"))
    }
}
