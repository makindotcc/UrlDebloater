package cc.makin.urldebloater.android

import android.content.Intent
import android.util.Patterns
import android.widget.Toast
import androidx.activity.compose.setContent
import androidx.annotation.StringRes
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.core.net.toUri
import cc.makin.urldebloater.android.ui.theme.UrlDebloaterTheme

private val linkIconSize = 20.dp
private val linkIconSpacing = 12.dp
private val linkRowPadding = 12.dp

class ClipboardOpenActivity : ClipboardWashActivity() {
    override val finishBeforeWash: Boolean
        get() = false

    override fun onCleaned(cleaned: String) {
        val links = extractLinks(cleaned)
        when (links.size) {
            0 -> finishWithToast(R.string.home_link_not_found)
            1 -> openLink(links.first())
            else -> askWhichLink(links)
        }
    }

    private fun extractLinks(text: String): List<String> {
        val matcher = Patterns.WEB_URL.matcher(text)
        val links = LinkedHashSet<String>()
        while (matcher.find()) {
            val raw = matcher.group()
            links += if (raw.startsWith("http://", true) || raw.startsWith("https://", true)) {
                raw
            } else {
                "https://$raw"
            }
        }
        return links.toList()
    }

    private fun openLink(link: String) {
        val intent = Intent(Intent.ACTION_VIEW, link.toUri())
        if (runCatching { startActivity(intent) }.isFailure) {
            finishWithToast(R.string.home_link_open_error)
            return
        }
        finish()
    }

    private fun askWhichLink(links: List<String>) {
        setContent {
            UrlDebloaterTheme {
                AlertDialog(
                    onDismissRequest = { finish() },
                    title = { Text(stringResource(R.string.home_link_chooser_title)) },
                    text = {
                        Column {
                            links.forEach { link ->
                                LinkRow(link = link, onClick = { openLink(link) })
                            }
                        }
                    },
                    confirmButton = {
                        TextButton(onClick = { finish() }) {
                            Text(stringResource(R.string.settings_cancel))
                        }
                    },
                )
            }
        }
    }

    @Composable
    private fun LinkRow(link: String, onClick: () -> Unit) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable(onClick = onClick)
                .padding(vertical = linkRowPadding),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(
                painter = painterResource(R.drawable.ic_open_in_new),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.primary,
                modifier = Modifier.size(linkIconSize),
            )
            Spacer(Modifier.width(linkIconSpacing))
            Text(
                text = link,
                style = MaterialTheme.typography.bodyLarge,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }

    private fun finishWithToast(@StringRes messageRes: Int) {
        Toast.makeText(this, messageRes, Toast.LENGTH_SHORT).show()
        finish()
    }
}
