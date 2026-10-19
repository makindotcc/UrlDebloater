package cc.makin.urldebloater.android

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Intent
import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.os.Bundle
import android.view.Gravity
import android.view.View
import android.widget.FrameLayout
import android.widget.ProgressBar
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.annotation.StringRes
import androidx.lifecycle.lifecycleScope
import cc.makin.urldebloater.android.settings.SettingsStore
import kotlinx.coroutines.launch

private const val progressWindowSizeDp = 56
private const val progressIndicatorSizeDp = 28
private const val progressCornerRadiusDp = 16

class TextWashActivity : ComponentActivity() {
    private var finished = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(progressView())
        window.setLayout(dp(progressWindowSizeDp), dp(progressWindowSizeDp))
        window.setGravity(Gravity.CENTER)
        washIncomingText()
    }

    private fun washIncomingText() {
        val processText = intent.action == Intent.ACTION_PROCESS_TEXT
        val raw = intent.getCharSequenceExtra(
            if (processText) Intent.EXTRA_PROCESS_TEXT else Intent.EXTRA_TEXT
        )?.toString()
        if (raw.isNullOrBlank()) {
            finishWithMessage(R.string.home_wash_empty_error, processText)
            return
        }

        val settings = SettingsStore(this).load()
        val app = application as UrlDebloaterApplication
        lifecycleScope.launch {
            val cleaned = when (val outcome = app.washOutcome(raw, settings)) {
                is WashOutcome.Cleaned -> outcome.text
                WashOutcome.InvalidMixerUrl -> {
                    finishWithMessage(R.string.settings_mixer_url_invalid, processText)
                    return@launch
                }
                WashOutcome.Failed -> {
                    finishWithMessage(R.string.home_wash_error, processText)
                    return@launch
                }
            }
            copyToClipboard(cleaned)
            if (processText) {
                setResult(
                    RESULT_OK,
                    Intent().putExtra(Intent.EXTRA_PROCESS_TEXT, cleaned),
                )
            }
            Toast.makeText(
                this@TextWashActivity,
                R.string.text_cleaned_message,
                Toast.LENGTH_SHORT,
            ).show()
            finished = true
            finish()
        }
    }

    private fun copyToClipboard(text: String) {
        getSystemService(ClipboardManager::class.java)
            .setPrimaryClip(ClipData.newPlainText(getString(R.string.app_name), text))
    }

    private fun finishWithMessage(@StringRes messageRes: Int, processText: Boolean) {
        if (finished) return
        finished = true
        Toast.makeText(this, messageRes, Toast.LENGTH_SHORT).show()
        if (processText) setResult(RESULT_CANCELED)
        finish()
    }

    private fun progressView(): View {
        val card = FrameLayout(this)
        card.background = GradientDrawable().apply {
            shape = GradientDrawable.RECTANGLE
            cornerRadius = dp(progressCornerRadiusDp).toFloat()
            setColor(Color.WHITE)
        }
        card.addView(
            ProgressBar(this),
            FrameLayout.LayoutParams(
                dp(progressIndicatorSizeDp),
                dp(progressIndicatorSizeDp),
                Gravity.CENTER,
            ),
        )
        return card
    }

    private fun dp(value: Int): Int = (value * resources.displayMetrics.density).toInt()
}
