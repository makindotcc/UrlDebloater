package cc.makin.urldebloater.android

import android.app.Activity
import android.content.Context
import android.content.Intent
import androidx.core.content.pm.ShortcutInfoCompat
import androidx.core.content.pm.ShortcutManagerCompat
import androidx.core.graphics.drawable.IconCompat

private const val clipboardShortcutId = "clipboard_wash"
private const val clipboardDynamicShortcutId = "clipboard_wash_action"
private const val openShortcutId = "clipboard_open"
private const val openDynamicShortcutId = "clipboard_open_action"

private fun clipboardShortcut(
    context: Context,
    id: String,
    labelRes: Int,
    iconRes: Int,
    target: Class<out Activity>,
): ShortcutInfoCompat {
    val label = context.getString(labelRes)
    return ShortcutInfoCompat.Builder(context, id)
        .setShortLabel(label)
        .setLongLabel(label)
        .setIcon(IconCompat.createWithResource(context, iconRes))
        .setIntent(Intent(Intent.ACTION_VIEW, null, context, target))
        .build()
}

fun publishClipboardShortcut(context: Context) {
    ShortcutManagerCompat.setDynamicShortcuts(
        context,
        listOf(
            clipboardShortcut(
                context,
                clipboardDynamicShortcutId,
                R.string.home_clean_clipboard_action,
                R.mipmap.ic_shortcut_clipboard,
                ClipboardWashActivity::class.java,
            ),
            clipboardShortcut(
                context,
                openDynamicShortcutId,
                R.string.home_clean_open_action,
                R.mipmap.ic_shortcut_open,
                ClipboardOpenActivity::class.java,
            ),
        ),
    )
}

fun isClipboardShortcutSupported(context: Context): Boolean =
    ShortcutManagerCompat.isRequestPinShortcutSupported(context)

fun requestClipboardShortcut(context: Context): Boolean {
    if (!isClipboardShortcutSupported(context)) return false
    return ShortcutManagerCompat.requestPinShortcut(
        context,
        clipboardShortcut(
            context,
            clipboardShortcutId,
            R.string.home_clean_clipboard_action,
            R.mipmap.ic_shortcut_clipboard,
            ClipboardWashActivity::class.java,
        ),
        null,
    )
}

fun requestClipboardOpenShortcut(context: Context): Boolean {
    if (!isClipboardShortcutSupported(context)) return false
    return ShortcutManagerCompat.requestPinShortcut(
        context,
        clipboardShortcut(
            context,
            openShortcutId,
            R.string.home_clean_open_action,
            R.mipmap.ic_shortcut_open,
            ClipboardOpenActivity::class.java,
        ),
        null,
    )
}
