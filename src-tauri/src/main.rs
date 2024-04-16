// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use lazy_static::lazy_static;
use std::sync::{Mutex};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Serialize, Deserialize, Debug)]
struct SerialPortConfig {
    data_bits: i8,
    stop_bits: i8,
    parity: String,
    flow_control: String,
}

lazy_static! {
    static ref SERIAL_PORT_CONFIG: Mutex<SerialPortConfig> = Mutex::new(SerialPortConfig{
        data_bits: 8,
        stop_bits: 1,
        parity: "None".to_string(),
        flow_control: "None".to_string(),
    });

    static ref PORTS: Mutex<HashMap<String, Box<dyn serialport::SerialPort>>> = Mutex::new(HashMap::new());
}

// 获取串口列表
#[tauri::command]
fn get_serial_port_list() -> Vec<String> {
    let mut vec: Vec<String> = Vec::new();
    let ports = serialport::available_ports().expect("No ports found!");
    for p in ports {
        vec.push(p.port_name);
    }
    vec
}

// 设置全局串口配置
#[tauri::command]
fn set_serial_port_config(data_bits: i8, stop_bits: i8, parity: String, flow_control: String) {
    let mut config = SERIAL_PORT_CONFIG.lock().unwrap();
    config.data_bits = data_bits;
    config.stop_bits = stop_bits;
    config.parity = parity;
    config.flow_control = flow_control;
}

// 获取全局串口设置
#[tauri::command]
fn get_serial_port_config() -> SerialPortConfig {
    let config = SERIAL_PORT_CONFIG.lock().unwrap();
    return SerialPortConfig {
        data_bits: config.data_bits,
        stop_bits: config.stop_bits,
        parity: config.parity.to_string(),
        flow_control: config.flow_control.to_string(),
    }
}

// 打开串口
#[tauri::command]
fn open_serial_port(port_name: &str, baud_rate: u32) {
    if !PORTS.lock().unwrap().contains_key(port_name) {
        if let Ok(port) = serialport::new(port_name, baud_rate)
            .timeout(Duration::from_millis(200))
            .open()
        {
            PORTS.lock().unwrap().insert(port_name.to_string(), port);
        } else {
            eprintln!("Failed to open serial port: {}", port_name);
        }
    }
}

// 写入数据到串口
#[tauri::command]
fn write_to_serial_port(port_name: &str, content: &str) -> bool {
    if let Some(port) = PORTS.lock().unwrap().get_mut(port_name) {
        let output = content.as_bytes();
        if let Ok(_) = port.write(output) {
            return true;
        }
    }
    return false;
}

// 从串口读取数据
#[tauri::command]
fn read_from_serial_port(port_name: &str) -> Vec<u8> {
    let mut serial_buf: Vec<u8> = Vec::new();
    if let Some(port) = PORTS.lock().unwrap().get_mut(port_name) {
        if let Err(e) = port.read(serial_buf.as_mut_slice()) {
            eprintln!("Read {}", e);
            return serial_buf;
        }
    }
    println!("{:X?}", serial_buf);
    serial_buf
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_serial_port_list,
            set_serial_port_config,
            get_serial_port_config,
            open_serial_port, 
            write_to_serial_port, 
            read_from_serial_port
            ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
