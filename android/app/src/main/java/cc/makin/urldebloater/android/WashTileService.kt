package cc.makin.urldebloater.android

import android.app.Activity
import android.app.PendingIntent
import android.content.Intent
import android.os.Build
import android.service.quicksettings.TileService
import androidx.annotation.RequiresApi

abstract class WashTileService : TileService() {
    protected abstract val target: Class<out Activity>

    override fun onClick() {
        if (isLocked) {
            unlockAndRun(::launchTarget)
        } else {
            launchTarget()
        }
    }

    private fun launchTarget() {
        val intent = Intent(this, target).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
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
