package cc.makin.urldebloater.android

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import cc.makin.urldebloater.android.rust.UrlWasherInitException
import cc.makin.urldebloater.android.settings.AppSettings
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch

private val homeContentMaxWidth = 760.dp
private val homeHorizontalPadding = 20.dp
private val homeTopPadding = 28.dp
private val homeBottomPadding = 32.dp
private val homeSectionSpacing = 20.dp
private val homeHeaderSpacing = 8.dp
private val homeActionSpacing = 12.dp
private val homeButtonHeight = 52.dp
private val homeProgressSize = 18.dp
private val homeProgressStrokeWidth = 2.dp
private val homeProgressSpacing = 8.dp
private val homeSnackbarPadding = 16.dp
private val homeErrorPadding = 12.dp
private const val homeTextFieldMinLines = 4
private const val homeTextFieldMaxLines = 8

private enum class HomeAction {
    CLEAN,
    CLEAN_AND_COPY,
}

@Composable
fun HomeScreen(
    settings: AppSettings,
    urlWasherAdapter: UrlWasherAdapter,
    modifier: Modifier = Modifier,
) {
    var input by rememberSaveable { mutableStateOf("") }
    var output by rememberSaveable { mutableStateOf("") }
    var errorRes by rememberSaveable { mutableStateOf<Int?>(null) }
    var runningAction by remember { mutableStateOf<HomeAction?>(null) }
    val scope = rememberCoroutineScope()
    val clipboardManager = LocalClipboardManager.current
    val snackbarHostState = remember { SnackbarHostState() }
    val copiedMessage = stringResource(R.string.home_copied_message)

    fun runAction(action: HomeAction) {
        if (runningAction != null || input.isBlank()) return

        runningAction = action
        output = ""
        errorRes = null
        scope.launch {
            var copied = false
            try {
                val cleaned = urlWasherAdapter.wash(input, settings)
                output = cleaned
                if (action == HomeAction.CLEAN_AND_COPY) {
                    try {
                        clipboardManager.setText(AnnotatedString(cleaned))
                        copied = true
                    } catch (_: Exception) {
                        errorRes = R.string.home_copy_error
                    }
                }
            } catch (cancellation: CancellationException) {
                throw cancellation
            } catch (_: UrlWasherInitException) {
                errorRes = R.string.settings_mixer_url_invalid
            } catch (_: Exception) {
                errorRes = R.string.home_wash_error
            } finally {
                runningAction = null
            }
            if (copied) {
                snackbarHostState.showSnackbar(copiedMessage)
            }
        }
    }

    Box(modifier = modifier.fillMaxSize()) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = homeHorizontalPadding),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Column(
                modifier = Modifier
                    .widthIn(max = homeContentMaxWidth)
                    .fillMaxWidth()
                    .padding(top = homeTopPadding, bottom = homeBottomPadding),
                verticalArrangement = Arrangement.spacedBy(homeSectionSpacing),
            ) {
                Column(verticalArrangement = Arrangement.spacedBy(homeHeaderSpacing)) {
                    Text(
                        text = stringResource(R.string.home_title),
                        style = MaterialTheme.typography.displaySmall,
                        fontWeight = FontWeight.Bold,
                    )
                    Text(
                        text = stringResource(R.string.home_subtitle),
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                OutlinedTextField(
                    value = input,
                    onValueChange = { input = it },
                    label = { Text(stringResource(R.string.home_input_label)) },
                    placeholder = { Text(stringResource(R.string.home_input_placeholder)) },
                    enabled = runningAction == null,
                    minLines = homeTextFieldMinLines,
                    maxLines = homeTextFieldMaxLines,
                    shape = MaterialTheme.shapes.extraLarge,
                    modifier = Modifier.fillMaxWidth(),
                )
                Row(
                    horizontalArrangement = Arrangement.spacedBy(homeActionSpacing),
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    FilledTonalButton(
                        onClick = { runAction(HomeAction.CLEAN) },
                        enabled = input.isNotBlank() && runningAction == null,
                        modifier = Modifier
                            .weight(1f)
                            .height(homeButtonHeight),
                    ) {
                        HomeActionContent(
                            isRunning = runningAction == HomeAction.CLEAN,
                            label = stringResource(R.string.home_wash_action),
                        )
                    }
                    Button(
                        onClick = { runAction(HomeAction.CLEAN_AND_COPY) },
                        enabled = input.isNotBlank() && runningAction == null,
                        modifier = Modifier
                            .weight(1f)
                            .height(homeButtonHeight),
                    ) {
                        HomeActionContent(
                            isRunning = runningAction == HomeAction.CLEAN_AND_COPY,
                            label = stringResource(R.string.home_clean_copy_action),
                        )
                    }
                }
                OutlinedTextField(
                    value = output,
                    onValueChange = {},
                    label = { Text(stringResource(R.string.home_output_label)) },
                    placeholder = { Text(stringResource(R.string.home_output_placeholder)) },
                    readOnly = true,
                    minLines = homeTextFieldMinLines,
                    maxLines = homeTextFieldMaxLines,
                    shape = MaterialTheme.shapes.extraLarge,
                    modifier = Modifier.fillMaxWidth(),
                )
                errorRes?.let {
                    Surface(
                        color = MaterialTheme.colorScheme.errorContainer,
                        contentColor = MaterialTheme.colorScheme.onErrorContainer,
                        shape = MaterialTheme.shapes.medium,
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Text(
                            text = stringResource(it),
                            style = MaterialTheme.typography.bodyMedium,
                            modifier = Modifier.padding(homeErrorPadding),
                        )
                    }
                }
            }
        }
        SnackbarHost(
            hostState = snackbarHostState,
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .padding(homeSnackbarPadding),
        )
    }
}

@Composable
private fun HomeActionContent(
    isRunning: Boolean,
    label: String,
) {
    if (isRunning) {
        CircularProgressIndicator(
            color = LocalContentColor.current,
            strokeWidth = homeProgressStrokeWidth,
            modifier = Modifier.size(homeProgressSize),
        )
        Spacer(Modifier.width(homeProgressSpacing))
    }
    Text(
        if (isRunning) stringResource(R.string.home_washing_action) else label
    )
}
