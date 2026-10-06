package app.luma.mobile.demo

import android.app.Activity
import android.content.Context
import android.media.AudioManager
import android.view.WindowManager
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Plugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject

@TauriPlugin
class PlayerDevicePlugin(private val activity: Activity) : Plugin(activity) {
  private val audio get() = activity.getSystemService(Context.AUDIO_SERVICE) as AudioManager

  @Command
  fun setPresentation(invoke: Invoke) {
    val args = invoke.getArgs()
    activity.runOnUiThread {
      try {
        val mainActivity = activity as? MainActivity ?: throw IllegalStateException("Player activity unavailable")
        mainActivity.setPlayerPresentation(args.optBoolean("active", false), args.optBoolean("controlsVisible", false))
        invoke.resolve()
      } catch (error: Exception) { invoke.reject("Could not update player presentation", error) }
    }
  }

  @Command
  fun getLevels(invoke: Invoke) {
    val maximum = audio.getStreamMaxVolume(AudioManager.STREAM_MUSIC).coerceAtLeast(1)
    val brightness = activity.window.attributes.screenBrightness
    invoke.resolve(JSObject().apply {
      put("volume", audio.getStreamVolume(AudioManager.STREAM_MUSIC) * 100.0 / maximum)
      put("brightness", if (brightness < 0) 50.0 else brightness * 100.0)
    })
  }

  @Command
  fun setLevels(invoke: Invoke) {
    val args = invoke.getArgs()
    activity.runOnUiThread {
      try {
        if (args.has("brightness")) {
          val params = activity.window.attributes
          params.screenBrightness = (args.getDouble("brightness") / 100.0).coerceIn(0.01, 1.0).toFloat()
          activity.window.attributes = params
        }
        if (args.has("volume")) {
          val max = audio.getStreamMaxVolume(AudioManager.STREAM_MUSIC)
          val level = (args.getDouble("volume").coerceIn(0.0,100.0) * max / 100.0).toInt()
          audio.setStreamVolume(AudioManager.STREAM_MUSIC, level, 0)
        }
        if (args.optBoolean("reset", false)) {
          val params = activity.window.attributes
          params.screenBrightness = WindowManager.LayoutParams.BRIGHTNESS_OVERRIDE_NONE
          activity.window.attributes = params
          activity.window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        } else {
          activity.window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        }
        invoke.resolve()
      } catch (error: Exception) { invoke.reject("Could not update player controls", error) }
    }
  }
}
