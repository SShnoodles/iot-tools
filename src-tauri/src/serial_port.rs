use std::collections::HashMap;
use std::sync::{Mutex};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use lazy_static::lazy_static;

#[derive(Serialize, Deserialize, Debug)]
pub struct SerialPortConfig {
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
pub fn get_serial_port_list() -> Vec<String> {
    let mut vec: Vec<String> = Vec::new();
    let ports = serialport::available_ports().expect("No ports found!");
    for p in ports {
        vec.push(p.port_name);
    }
    vec
}

// 设置全局串口配置
#[tauri::command]
pub fn set_serial_port_config(data_bits: i8, stop_bits: i8, parity: String, flow_control: String) {
    let mut config = SERIAL_PORT_CONFIG.lock().unwrap();
    config.data_bits = data_bits;
    config.stop_bits = stop_bits;
    config.parity = parity;
    config.flow_control = flow_control;
}

// 获取全局串口设置
#[tauri::command]
pub fn get_serial_port_config() -> SerialPortConfig {
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
pub fn open_serial_port(port_name: &str, baud_rate: u32) -> Result<String, String>{
    if PORTS.lock().unwrap().contains_key(port_name) {
        return Ok("Opened".to_string());
    }
    let s = serialport::new(port_name, baud_rate)
        .timeout(Duration::from_millis(200))
        .open();
    return match s {
        Ok(port) => {
            PORTS.lock().unwrap().insert(port_name.to_string(), port);
            Ok("Opened".to_string())
        },
        Err(e) => Err(e.description),
    };
}

// 关闭串口
#[tauri::command]
pub fn stop_serial_port(port_name: &str) {
    if PORTS.lock().unwrap().contains_key(port_name) {
        PORTS.lock().unwrap().remove(port_name);
        drop(PORTS.lock().unwrap().get_mut(port_name));
    }
}

// 写入数据到串口
#[tauri::command]
pub fn write_to_serial_port(port_name: &str, content: Vec<u8>) -> bool {
    if let Some(port) = PORTS.lock().unwrap().get_mut(port_name) {
        if let Ok(_) = port.write(&content) {
            return true;
        }
    }
    return false;
}

// 从串口读取数据
#[tauri::command]
pub fn read_from_serial_port(port_name: &str) -> Vec<u8> {
    let mut serial_buf: Vec<u8> = Vec::new();
    if let Some(port) = PORTS.lock().unwrap().get_mut(port_name) {
        if let Err(_) = port.read(serial_buf.as_mut_slice()) {
            return serial_buf;
        }
    }
    serial_buf
}

// 检查串口是否已打开
#[tauri::command]
pub fn is_serial_port_open(port_name: &str) -> bool {
    if let Some(_) = PORTS.lock().unwrap().get_mut(port_name) {
        return true
    } else {
        return false
    }
}