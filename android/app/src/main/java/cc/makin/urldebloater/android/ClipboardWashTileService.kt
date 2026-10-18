package cc.makin.urldebloater.android

import android.app.PendingIntent
import android.content.Intent
import android.os.Build
import android.service.quicksettings.TileService
import androidx.annotation.RequiresApi

class ClipboardWashTileService : TileService() {
    override fun onClick() {
        if (isLocked) {
            unlockAndRun { launchClipboardWash() }
        } else {
            launchClipboardWash()
        }
    }

    private fun launchClipboardWash() {
        val intent = Intent(this, ClipboardWashActivity::class.java)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            startWithPendingIntent(intent)
        } else {
            startWithIntent(intent)
        }
    }

    // The PendingIntent overload exists only on API 34+; older platforms accept an Intent only.
    @Suppress("DEPRECATION", "StartActivityAndCollapseDeprecated")
    private fun startWithIntent(intent: Intent) {
        startActivityAndCollapse(intent)
    }

    @RequiresApi(Build.VERSION_CODES.UPSIDE_DOWN_CAKE)
    private fun startWithPendingIntent(intent: Intent) {
        startActivityAndCollapse(
            PendingIntent.getActivity(
                this,
                0,
                intent,
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            )
        )
    }
}
