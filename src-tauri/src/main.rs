// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod serial_port;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
                serial_port::get_serial_port_list,
                serial_port::set_serial_port_config,
                serial_port::get_serial_port_config,
                serial_port::open_serial_port,
                serial_port::write_to_serial_port,
                serial_port::read_from_serial_port,
                serial_port::is_serial_port_open
            ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
