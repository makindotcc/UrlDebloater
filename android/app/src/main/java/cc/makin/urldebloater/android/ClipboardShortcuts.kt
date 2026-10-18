package cc.makin.urldebloater.android

import android.content.Context
import android.content.Intent
import androidx.core.content.pm.ShortcutInfoCompat
import androidx.core.content.pm.ShortcutManagerCompat
import androidx.core.graphics.drawable.IconCompat

private const val clipboardShortcutId = "clipboard_wash"
private const val clipboardDynamicShortcutId = "clipboard_wash_action"

private fun clipboardShortcut(context: Context, id: String): ShortcutInfoCompat {
    val label = context.getString(R.string.home_clean_clipboard_action)
    return ShortcutInfoCompat.Builder(context, id)
        .setShortLabel(label)
        .setLongLabel(label)
        .setIcon(IconCompat.createWithResource(context, R.mipmap.ic_shortcut_clipboard))
        .setIntent(Intent(Intent.ACTION_VIEW, null, context, ClipboardWashActivity::class.java))
        .build()
}

fun publishClipboardShortcut(context: Context) {
    ShortcutManagerCompat.setDynamicShortcuts(
        context,
        listOf(clipboardShortcut(context, clipboardDynamicShortcutId)),
    )
}

fun isClipboardShortcutSupported(context: Context): Boolean =
    ShortcutManagerCompat.isRequestPinShortcutSupported(context)

fun requestClipboardShortcut(context: Context): Boolean {
    if (!isClipboardShortcutSupported(context)) return false
    return ShortcutManagerCompat.requestPinShortcut(
        context,
        clipboardShortcut(context, clipboardShortcutId),
        null,
    )
}
