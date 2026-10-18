package cc.makin.urldebloater.android.settings

import androidx.annotation.DrawableRes
import androidx.compose.foundation.LocalIndication
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import cc.makin.urldebloater.android.R
import cc.makin.urldebloater.android.ui.theme.UrlDebloaterTheme

private const val DISABLED_CONTENT_ALPHA = 0.38f
private val settingsContentMaxWidth = 840.dp
private val settingsHorizontalPadding = 20.dp
private val settingsSectionSpacing = 24.dp
private val preferenceIconContainerSize = 40.dp
private val preferenceIconSize = 20.dp
private val preferenceMinHeight = 72.dp
private val resetButtonHeight = 56.dp

private enum class SettingsDialog {
    REDIRECT_POLICY,
    MIXER_URL,
    TWITTER_POLICY,
    RESET_DEFAULTS,
}

@Composable
fun SettingsScreen(
    initialSettings: AppSettings,
    onSettingsPersist: (AppSettings) -> Unit,
    modifier: Modifier = Modifier,
) {
    var settings by remember { mutableStateOf(initialSettings) }
    var openDialog by rememberSaveable { mutableStateOf<SettingsDialog?>(null) }

    fun update(transform: (AppSettings) -> AppSettings) {
        val updated = transform(settings)
        settings = updated
        onSettingsPersist(updated)
    }

    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState()),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Column(
            modifier = Modifier
                .widthIn(max = settingsContentMaxWidth)
                .fillMaxWidth()
                .padding(horizontal = settingsHorizontalPadding),
        ) {
            Text(
                text = stringResource(R.string.navbar_settings),
                style = MaterialTheme.typography.displaySmall,
                fontWeight = FontWeight.Bold,
                modifier = Modifier.padding(top = 24.dp, bottom = 32.dp),
            )
            SettingsSection(stringResource(R.string.settings_section_general)) {
                SwitchPreference(
                    iconRes = R.drawable.ic_content_cut,
                    title = stringResource(R.string.settings_clipboard_debloater),
                    checked = settings.clipboardDebloater,
                    onCheckedChange = { checked ->
                        update { it.copy(clipboardDebloater = checked) }
                    },
                )
                SettingsDivider()
                SwitchPreference(
                    iconRes = R.drawable.ic_database,
                    title = stringResource(R.string.settings_use_clear_urls),
                    checked = settings.useClearUrls,
                    onCheckedChange = { checked -> update { it.copy(useClearUrls = checked) } },
                )
            }
            SettingsSection(stringResource(R.string.settings_section_redirects)) {
                SettingsPreference(
                    iconRes = R.drawable.ic_conversion_path,
                    title = stringResource(R.string.settings_redirect_wash_policy),
                    summary = stringResource(settings.redirectWashPolicy.labelRes),
                    onClick = { openDialog = SettingsDialog.REDIRECT_POLICY },
                )
                SettingsDivider()
                SettingsPreference(
                    iconRes = R.drawable.ic_cloud,
                    title = stringResource(R.string.settings_mixer_url),
                    summary = settings.mixerUrl,
                    enabled = settings.redirectWashPolicy == RedirectWashPolicy.VIA_MIXER,
                    onClick = { openDialog = SettingsDialog.MIXER_URL },
                )
            }
            SettingsSection(stringResource(R.string.settings_section_custom_domains)) {
                SettingsPreference(
                    iconRes = R.drawable.ic_language,
                    title = stringResource(R.string.settings_twitter_domain),
                    summary = stringResource(settings.twitterPolicy.labelRes),
                    onClick = { openDialog = SettingsDialog.TWITTER_POLICY },
                )
            }
            FilledTonalButton(
                onClick = { openDialog = SettingsDialog.RESET_DEFAULTS },
                modifier = Modifier
                    .fillMaxWidth()
                    .height(resetButtonHeight),
                shape = MaterialTheme.shapes.extraLarge,
                colors = ButtonDefaults.filledTonalButtonColors(
                    containerColor = MaterialTheme.colorScheme.errorContainer,
                    contentColor = MaterialTheme.colorScheme.onErrorContainer,
                ),
            ) {
                Icon(
                    painter = painterResource(R.drawable.ic_restart_alt),
                    contentDescription = null,
                )
                Spacer(Modifier.width(8.dp))
                Text(stringResource(R.string.settings_reset_defaults))
            }
            Spacer(Modifier.height(32.dp))
        }
    }

    when (openDialog) {
        SettingsDialog.REDIRECT_POLICY -> SingleChoiceDialog(
            title = stringResource(R.string.settings_redirect_wash_policy),
            options = RedirectWashPolicy.entries,
            selected = settings.redirectWashPolicy,
            optionLabel = { stringResource(it.labelRes) },
            supportingText = stringResource(R.string.settings_policy_via_mixer_privacy_notice),
            onSelect = { policy ->
                update { it.copy(redirectWashPolicy = policy) }
                openDialog = null
            },
            onDismiss = { openDialog = null },
        )

        SettingsDialog.MIXER_URL -> MixerUrlDialog(
            currentUrl = settings.mixerUrl,
            onDismiss = { openDialog = null },
            onSave = { url ->
                update { it.copy(mixerUrl = url) }
                openDialog = null
            },
        )

        SettingsDialog.TWITTER_POLICY -> SingleChoiceDialog(
            title = stringResource(R.string.settings_twitter_domain),
            options = TwitterPolicy.entries,
            selected = settings.twitterPolicy,
            optionLabel = { stringResource(it.labelRes) },
            onSelect = { policy ->
                update { it.copy(twitterPolicy = policy) }
                openDialog = null
            },
            onDismiss = { openDialog = null },
        )

        SettingsDialog.RESET_DEFAULTS -> AlertDialog(
            onDismissRequest = { openDialog = null },
            icon = {
                Icon(
                    painter = painterResource(R.drawable.ic_restart_alt),
                    contentDescription = null,
                )
            },
            title = { Text(stringResource(R.string.settings_reset_defaults)) },
            text = { Text(stringResource(R.string.settings_reset_confirmation)) },
            confirmButton = {
                TextButton(
                    onClick = {
                        val defaults = AppSettings()
                        settings = defaults
                        onSettingsPersist(defaults)
                        openDialog = null
                    },
                    colors = ButtonDefaults.textButtonColors(
                        contentColor = MaterialTheme.colorScheme.error,
                    ),
                ) {
                    Text(stringResource(R.string.settings_reset_confirm))
                }
            },
            dismissButton = {
                TextButton(onClick = { openDialog = null }) {
                    Text(stringResource(R.string.settings_cancel))
                }
            },
        )

        null -> Unit
    }
}

@Composable
private fun SettingsSection(
    title: String,
    content: @Composable ColumnScope.() -> Unit,
) {
    Text(
        text = title,
        style = MaterialTheme.typography.titleMedium,
        fontWeight = FontWeight.Bold,
        color = MaterialTheme.colorScheme.primary,
        modifier = Modifier.padding(start = 12.dp, bottom = 8.dp),
    )
    Surface(
        modifier = Modifier.fillMaxWidth(),
        shape = MaterialTheme.shapes.extraLarge,
        color = MaterialTheme.colorScheme.surfaceContainerLow,
    ) {
        Column(content = content)
    }
    Spacer(Modifier.height(settingsSectionSpacing))
}

@Composable
private fun SettingsDivider() {
    HorizontalDivider(
        modifier = Modifier.padding(start = 72.dp),
        color = MaterialTheme.colorScheme.outlineVariant,
    )
}

@Composable
private fun PreferenceIcon(
    @DrawableRes iconRes: Int,
    enabled: Boolean = true,
) {
    Surface(
        modifier = Modifier
            .size(preferenceIconContainerSize)
            .alpha(if (enabled) 1f else DISABLED_CONTENT_ALPHA),
        shape = MaterialTheme.shapes.medium,
        color = MaterialTheme.colorScheme.primaryContainer,
    ) {
        Box(
            modifier = Modifier.fillMaxSize(),
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                painter = painterResource(iconRes),
                contentDescription = null,
                modifier = Modifier.size(preferenceIconSize),
                tint = MaterialTheme.colorScheme.onPrimaryContainer,
            )
        }
    }
}

@Composable
private fun SwitchPreference(
    @DrawableRes iconRes: Int,
    title: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
) {
    val interactionSource = remember { MutableInteractionSource() }
    val indication = LocalIndication.current

    ListItem(
        headlineContent = { Text(title) },
        leadingContent = { PreferenceIcon(iconRes) },
        trailingContent = {
            Switch(
                checked = checked,
                onCheckedChange = null,
                interactionSource = interactionSource,
            )
        },
        colors = ListItemDefaults.colors(containerColor = Color.Transparent),
        modifier = Modifier
            .heightIn(min = preferenceMinHeight)
            .toggleable(
                value = checked,
                interactionSource = interactionSource,
                indication = indication,
                role = Role.Switch,
                onValueChange = onCheckedChange,
            ),
    )
}

@Composable
private fun SettingsPreference(
    @DrawableRes iconRes: Int,
    title: String,
    onClick: () -> Unit,
    summary: String? = null,
    enabled: Boolean = true,
) {
    val contentAlpha = if (enabled) 1f else DISABLED_CONTENT_ALPHA

    ListItem(
        headlineContent = { Text(title) },
        leadingContent = { PreferenceIcon(iconRes, enabled) },
        supportingContent = summary?.let {
            {
                Text(
                    text = it,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        },
        trailingContent = {
            Icon(
                painter = painterResource(R.drawable.ic_chevron_right),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = contentAlpha),
            )
        },
        colors = ListItemDefaults.colors(
            containerColor = Color.Transparent,
            headlineColor = MaterialTheme.colorScheme.onSurface.copy(alpha = contentAlpha),
            supportingColor = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = contentAlpha),
        ),
        modifier = Modifier
            .heightIn(min = preferenceMinHeight)
            .clickable(enabled = enabled, role = Role.Button, onClick = onClick),
    )
}

@Composable
private fun <T> SingleChoiceDialog(
    title: String,
    options: List<T>,
    selected: T,
    optionLabel: @Composable (T) -> String,
    supportingText: String? = null,
    onSelect: (T) -> Unit,
    onDismiss: () -> Unit,
) {
    Dialog(onDismissRequest = onDismiss) {
        Card(
            shape = MaterialTheme.shapes.extraLarge,
            colors = CardDefaults.cardColors(
                containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
            ),
            modifier = Modifier.fillMaxWidth(),
        ) {
            Text(
                text = title,
                style = MaterialTheme.typography.headlineSmall,
                modifier = Modifier.padding(start = 24.dp, end = 24.dp, top = 24.dp, bottom = 8.dp),
            )
            Column(
                modifier = Modifier
                    .selectableGroup()
                    .padding(bottom = 12.dp),
            ) {
                options.forEach { option ->
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .selectable(
                                selected = option == selected,
                                role = Role.RadioButton,
                                onClick = { onSelect(option) },
                            )
                            .padding(horizontal = 24.dp, vertical = 12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        RadioButton(selected = option == selected, onClick = null)
                        Spacer(Modifier.width(16.dp))
                        Text(
                            text = optionLabel(option),
                            style = MaterialTheme.typography.bodyLarge,
                        )
                    }
                }
            }
            if (supportingText != null) {
                Text(
                    text = supportingText,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(start = 24.dp, end = 24.dp, bottom = 24.dp),
                )
            }
        }
    }
}

@Composable
private fun MixerUrlDialog(
    currentUrl: String,
    onDismiss: () -> Unit,
    onSave: (String) -> Unit,
) {
    var url by rememberSaveable { mutableStateOf(currentUrl) }
    val urlValid = isValidMixerUrl(url)

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.settings_mixer_url)) },
        text = {
            Column {
                OutlinedTextField(
                    value = url,
                    onValueChange = { url = it },
                    isError = !urlValid,
                    supportingText = {
                        if (!urlValid) {
                            Text(stringResource(R.string.settings_mixer_url_invalid))
                        }
                    },
                    singleLine = true,
                    shape = MaterialTheme.shapes.large,
                    keyboardOptions = KeyboardOptions(
                        keyboardType = KeyboardType.Uri,
                        imeAction = ImeAction.Done,
                    ),
                    modifier = Modifier.fillMaxWidth(),
                )
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.End,
                ) {
                    TextButton(
                        onClick = { url = PUBLIC_MIXER_INSTANCE },
                        enabled = url.trim() != PUBLIC_MIXER_INSTANCE,
                    ) {
                        Text(stringResource(R.string.settings_mixer_url_default))
                    }
                }
            }
        },
        confirmButton = {
            TextButton(
                onClick = { onSave(url.trim()) },
                enabled = urlValid,
            ) {
                Text(stringResource(R.string.settings_save))
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(stringResource(R.string.settings_cancel))
            }
        },
    )
}

@Preview(showBackground = true, heightDp = 800)
@Composable
private fun SettingsScreenPreview() {
    UrlDebloaterTheme {
        SettingsScreen(AppSettings(), onSettingsPersist = {})
    }
}
