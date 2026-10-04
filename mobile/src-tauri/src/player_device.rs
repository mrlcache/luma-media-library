use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager,
};

#[cfg(target_os = "android")]
struct PlayerDevice(tauri::plugin::PluginHandle<tauri::Wry>);

pub fn init() -> TauriPlugin<tauri::Wry> {
    Builder::new("player-device")
        .setup(|app, api| {
            #[cfg(target_os = "android")]
            app.manage(PlayerDevice(api.register_android_plugin(
                "app.luma.mobile.demo",
                "PlayerDevicePlugin",
            )?));
            #[cfg(not(target_os = "android"))]
            let _ = (app, api);
            Ok(())
        })
        .build()
}

#[tauri::command]
pub async fn mobile_player_levels(
    app: tauri::AppHandle,
    values: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    #[cfg(target_os = "android")]
    {
        let state = app.state::<PlayerDevice>();
        let (command, args) = match values {
            Some(value) => ("setLevels", value),
            None => ("getLevels", serde_json::json!({})),
        };
        state
            .0
            .run_mobile_plugin_async(command, args)
            .await
            .map_err(|error| error.to_string())
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, values);
        Ok(serde_json::Value::Null)
    }
}
