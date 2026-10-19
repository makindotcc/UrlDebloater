package cc.makin.urldebloater.android

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.widget.Toast
import androidx.core.app.NotificationCompat
import cc.makin.urldebloater.android.settings.SettingsStore
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch

private const val extraText = "text"
private const val washChannelId = "clipboard_wash"
private const val washNotificationId = 1

class ClipboardWashService : Service() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private var washJob: Job? = null
    private var activeStartId = 0

    companion object {
        fun intent(context: Context, text: String): Intent =
            Intent(context, ClipboardWashService::class.java).putExtra(extraText, text)
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val notification = buildNotification()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            startForeground(
                washNotificationId,
                notification,
                ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC,
            )
        } else {
            startForeground(washNotificationId, notification)
        }
        activeStartId = startId

        val text = intent?.getStringExtra(extraText)
        if (text.isNullOrBlank()) {
            stopForeground(STOP_FOREGROUND_REMOVE)
            stopSelfResult(startId)
            return START_NOT_STICKY
        }

        washJob?.cancel()
        washJob = scope.launch {
            try {
                val settings = SettingsStore(this@ClipboardWashService).load()
                val cleaned = (application as UrlDebloaterApplication)
                    .washClipboard(this@ClipboardWashService, text, settings)
                if (cleaned != null) {
                    Toast.makeText(
                        this@ClipboardWashService,
                        R.string.home_clipboard_cleaned_message,
                        Toast.LENGTH_SHORT,
                    ).show()
                }
            } finally {
                if (startId == activeStartId) {
                    stopForeground(STOP_FOREGROUND_REMOVE)
                    stopSelfResult(startId)
                }
            }
        }
        return START_NOT_STICKY
    }

    private fun buildNotification(): Notification {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            getSystemService(NotificationManager::class.java).createNotificationChannel(
                NotificationChannel(
                    washChannelId,
                    getString(R.string.wash_progress_title),
                    NotificationManager.IMPORTANCE_LOW,
                )
            )
        }
        return NotificationCompat.Builder(this, washChannelId)
            .setSmallIcon(R.drawable.ic_content_cut)
            .setContentTitle(getString(R.string.wash_progress_title))
            .setContentText(getString(R.string.wash_progress_text))
            .setOngoing(true)
            .setSilent(true)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .build()
    }

    override fun onDestroy() {
        scope.cancel()
        super.onDestroy()
    }
}
