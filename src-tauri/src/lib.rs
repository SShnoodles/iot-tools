// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
pub mod display;
pub mod iec104;
pub mod modbus_tcp;
pub mod mqtt;
pub mod opcua;
pub mod serial_port;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{async_runtime, Emitter};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // macOS routes the standard editing shortcuts through native menu items.
            // Keep these predefined actions whenever a custom application menu is used,
            // otherwise Cmd+X/C/V/A/Z stops working in every webview input field.
            let undo = PredefinedMenuItem::undo(app, None)?;
            let redo = PredefinedMenuItem::redo(app, None)?;
            let cut = PredefinedMenuItem::cut(app, None)?;
            let copy = PredefinedMenuItem::copy(app, None)?;
            let paste = PredefinedMenuItem::paste(app, None)?;
            let select_all = PredefinedMenuItem::select_all(app, None)?;
            let edit_separator = PredefinedMenuItem::separator(app)?;
            let edit_menu = Submenu::with_items(
                app,
                "Edit",
                true,
                &[
                    &undo,
                    &redo,
                    &edit_separator,
                    &cut,
                    &copy,
                    &paste,
                    &select_all,
                ],
            )?;

            let lang_zh = MenuItem::with_id(app, "lang-zh", "中文", true, None::<&str>)?;
            let lang_en = MenuItem::with_id(app, "lang-en", "English", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let version_label = format!("v{}", env!("CARGO_PKG_VERSION"));
            let version_item =
                MenuItem::with_id(app, "version", &version_label, true, None::<&str>)?;
            let lang_menu = Submenu::with_items(
                app,
                "Language",
                true,
                &[&lang_zh, &lang_en, &sep, &version_item],
            )?;
            let menu = Menu::with_items(app, &[&edit_menu, &lang_menu])?;
            app.set_menu(menu)?;
            app.on_menu_event(|app, event| match event.id().as_ref() {
                "lang-zh" => {
                    app.emit("lang-change", "zh").unwrap();
                }
                "lang-en" => {
                    app.emit("lang-change", "en").unwrap();
                }
                "version" => {
                    let app = app.clone();
                    async_runtime::spawn(async move {
                        match fetch_latest_version().await {
                            Ok(latest) => {
                                let current = env!("CARGO_PKG_VERSION");
                                if latest.trim_start_matches('v') != current {
                                    app.emit("update-available", latest).unwrap();
                                } else {
                                    app.emit("update-up-to-date", current).unwrap();
                                }
                            }
                            Err(e) => {
                                app.emit("update-check-failed", e).unwrap();
                            }
                        }
                    });
                }
                _ => {}
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            serial_port::get_serial_port_list,
            serial_port::set_serial_port_config,
            serial_port::get_serial_port_config,
            serial_port::open_serial_port,
            serial_port::stop_serial_port,
            serial_port::write_to_serial_port,
            serial_port::is_serial_port_open,
            modbus_tcp::modbus_tcp_connect,
            modbus_tcp::modbus_tcp_disconnect,
            modbus_tcp::modbus_tcp_is_connected,
            modbus_tcp::modbus_tcp_send,
            mqtt::mqtt_connect,
            mqtt::mqtt_disconnect,
            mqtt::mqtt_is_connected,
            mqtt::mqtt_subscribe,
            mqtt::mqtt_unsubscribe,
            mqtt::mqtt_publish,
            opcua::opcua_connect,
            opcua::opcua_disconnect,
            opcua::opcua_is_connected,
            opcua::opcua_read_node,
            opcua::opcua_write_node,
            iec104::iec104_connect,
            iec104::iec104_disconnect,
            iec104::iec104_is_connected,
            iec104::iec104_send_control,
            iec104::iec104_general_interrogation,
            iec104::iec104_clock_sync,
            iec104::iec104_send_raw,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn fetch_latest_version() -> Result<String, String> {
    let client = reqwest::Client::builder()
        .user_agent("iot-tools")
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get("https://api.github.com/repos/SShnoodles/iot-tools/releases/latest")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    json["tag_name"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "No release found".to_string())
}
