package cc.makin.urldebloater.android

import android.graphics.Color
import android.os.Build
import android.os.Bundle
import androidx.annotation.StringRes
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.windowInsetsBottomHeight
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.adaptive.navigationsuite.NavigationSuiteScaffold
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.PreviewScreenSizes
import cc.makin.urldebloater.android.settings.AppSettings
import cc.makin.urldebloater.android.settings.SettingsScreen
import cc.makin.urldebloater.android.settings.SettingsStore
import cc.makin.urldebloater.android.ui.theme.UrlDebloaterTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge(
            navigationBarStyle = SystemBarStyle.auto(Color.TRANSPARENT, Color.TRANSPARENT)
        )
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            window.isNavigationBarContrastEnforced = false
        }
        val app = application as UrlDebloaterApplication
        setContent {
            UrlDebloaterTheme {
                Box(modifier = Modifier.fillMaxSize()) {
                    UrlDebloaterApp(
                        wash = { text, settings -> app.washOutcome(text, settings) },
                    )
                    Spacer(
                        modifier = Modifier
                            .align(Alignment.BottomCenter)
                            .fillMaxWidth()
                            .windowInsetsBottomHeight(WindowInsets.navigationBars)
                            .background(MaterialTheme.colorScheme.surfaceContainer)
                    )
                }
            }
        }
    }
}

@PreviewScreenSizes
@Composable
private fun UrlDebloaterAppPreview() {
    UrlDebloaterApp(wash = { text, _ -> WashOutcome.Cleaned(text) })
}

@Composable
fun UrlDebloaterApp(wash: suspend (String, AppSettings) -> WashOutcome) {
    var currentDestination by rememberSaveable { mutableStateOf(AppDestinations.HOME) }
    val context = LocalContext.current
    val settingsStore = remember { SettingsStore(context.applicationContext) }
    var settings by remember { mutableStateOf(settingsStore.load()) }

    NavigationSuiteScaffold(
        navigationSuiteItems = {
            AppDestinations.entries.forEach {
                item(
                    icon = {
                        Icon(
                            painterResource(it.icon),
                            contentDescription = stringResource(it.labelRes)
                        )
                    },
                    label = { Text(stringResource(it.labelRes)) },
                    selected = it == currentDestination,
                    onClick = { currentDestination = it }
                )
            }
        }
    ) {
        Scaffold(modifier = Modifier.fillMaxSize()) { innerPadding ->
            when (currentDestination) {
                AppDestinations.HOME -> HomeScreen(
                    settings = settings,
                    wash = wash,
                    modifier = Modifier.padding(innerPadding),
                )

                AppDestinations.SETTINGS -> SettingsScreen(
                    initialSettings = settings,
                    onSettingsPersist = { updatedSettings ->
                        settingsStore.save(updatedSettings)
                        settings = updatedSettings
                    },
                    modifier = Modifier.padding(innerPadding),
                )
            }
        }
    }
}

enum class AppDestinations(
    @param:StringRes val labelRes: Int,
    val icon: Int,
) {
    HOME(R.string.navbar_home, R.drawable.ic_home),
    SETTINGS(R.string.navbar_settings, R.drawable.ic_settings),
}

