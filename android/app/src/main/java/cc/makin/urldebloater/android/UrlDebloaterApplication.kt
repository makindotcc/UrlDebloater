package cc.makin.urldebloater.android

import android.app.Application

class UrlDebloaterApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        NativeBridge.initTls(applicationContext)
        publishClipboardShortcut(this)
    }
}
