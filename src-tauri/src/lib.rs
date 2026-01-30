// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
pub mod serial_port;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            serial_port::get_serial_port_list,
            serial_port::set_serial_port_config,
            serial_port::get_serial_port_config,
            serial_port::open_serial_port,
            serial_port::stop_serial_port,
            serial_port::write_to_serial_port,
            serial_port::read_from_serial_port,
            serial_port::is_serial_port_open,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
