package cc.makin.urldebloater.android

import android.content.Context
import android.content.Intent
import androidx.core.content.pm.ShortcutInfoCompat
import androidx.core.content.pm.ShortcutManagerCompat
import androidx.core.graphics.drawable.IconCompat

private const val clipboardShortcutId = "clipboard_wash"

fun isClipboardShortcutSupported(context: Context): Boolean =
    ShortcutManagerCompat.isRequestPinShortcutSupported(context)

fun requestClipboardShortcut(context: Context): Boolean {
    if (!isClipboardShortcutSupported(context)) return false
    val label = context.getString(R.string.home_clean_clipboard_action)
    val shortcut = ShortcutInfoCompat.Builder(context, clipboardShortcutId)
        .setShortLabel(label)
        .setLongLabel(label)
        .setIcon(IconCompat.createWithResource(context, R.mipmap.ic_shortcut_clipboard))
        .setIntent(Intent(Intent.ACTION_VIEW, null, context, ClipboardWashActivity::class.java))
        .build()
    return ShortcutManagerCompat.requestPinShortcut(context, shortcut, null)
}
