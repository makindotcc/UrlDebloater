package cc.makin.urldebloater.android

import android.content.Context

object NativeBridge {
    init {
        System.loadLibrary("urldebloater")
    }

    @JvmStatic
    external fun initTls(context: Context)
}
