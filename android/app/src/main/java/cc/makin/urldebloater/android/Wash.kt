package cc.makin.urldebloater.android

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.widget.Toast
import androidx.annotation.StringRes
import cc.makin.urldebloater.android.rust.UrlWasherInitException
import cc.makin.urldebloater.android.settings.AppSettings
import kotlinx.coroutines.CancellationException

sealed interface WashOutcome {
    data class Cleaned(val text: String) : WashOutcome
    data object InvalidMixerUrl : WashOutcome
    data object Failed : WashOutcome
}

internal suspend fun UrlWasherAdapter.washOutcome(
    text: String,
    settings: AppSettings,
): WashOutcome = try {
    WashOutcome.Cleaned(wash(text, settings))
} catch (cancellation: CancellationException) {
    throw cancellation
} catch (_: UrlWasherInitException) {
    WashOutcome.InvalidMixerUrl
} catch (_: Exception) {
    WashOutcome.Failed
}

internal suspend fun Context.washClipboardText(
    washer: UrlWasherAdapter,
    text: String,
    settings: AppSettings,
    @StringRes failureMessageRes: Int = R.string.home_clipboard_error,
): String? {
    val cleaned = when (val outcome = washer.washOutcome(text, settings)) {
        is WashOutcome.Cleaned -> outcome.text
        WashOutcome.InvalidMixerUrl -> {
            showWashError(R.string.settings_mixer_url_invalid)
            return null
        }
        WashOutcome.Failed -> {
            showWashError(failureMessageRes)
            return null
        }
    }
    getSystemService(ClipboardManager::class.java).setPrimaryClip(
        ClipData.newPlainText(getString(R.string.app_name), cleaned)
    )
    return cleaned
}

private fun Context.showWashError(@StringRes messageRes: Int) {
    Toast.makeText(this, messageRes, Toast.LENGTH_SHORT).show()
}
