use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{Emitter, Manager};

#[derive(Serialize, Deserialize, Debug)]
pub struct SerialPortConfig {
    data_bits: i8,
    stop_bits: i8,
    parity: String,
    flow_control: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerialPortLog {
    pub direction: String,     // "RX" or "TX"
    pub content_hex: String,   // HEX format
    pub content_ascii: String, // ASCII format
    pub timestamp: String,
    pub new_group: bool, // true 表示距上次超过 50ms，前端可换行分组
}

static SERIAL_PORT_CONFIG: Lazy<Mutex<SerialPortConfig>> = Lazy::new(|| {
    Mutex::new(SerialPortConfig {
        data_bits: 8,
        stop_bits: 1,
        parity: "None".to_string(),
        flow_control: "None".to_string(),
    })
});
static PORTS: Lazy<Mutex<HashMap<String, Box<dyn serialport::SerialPort>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static SERIAL_PORT_ACTIVE: Lazy<Mutex<HashMap<String, Arc<AtomicBool>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn convert_to_data_bits(bits: i8) -> serialport::DataBits {
    match bits {
        5 => serialport::DataBits::Five,
        6 => serialport::DataBits::Six,
        7 => serialport::DataBits::Seven,
        8 => serialport::DataBits::Eight,
        _ => panic!("Invalid number of data bits"),
    }
}

pub fn convert_to_stop_bits(bits: i8) -> serialport::StopBits {
    match bits {
        1 => serialport::StopBits::One,
        2 => serialport::StopBits::Two,
        _ => panic!("Invalid number of stop bits"),
    }
}

pub fn convert_to_parity(s: &str) -> serialport::Parity {
    match s {
        "None" => serialport::Parity::None,
        "Odd" => serialport::Parity::Odd,
        "Even" => serialport::Parity::Even,
        _ => panic!("Invalid string of parity"),
    }
}

pub fn convert_to_flow_control(s: &str) -> serialport::FlowControl {
    match s {
        "None" => serialport::FlowControl::None,
        "Software" => serialport::FlowControl::Software,
        "Hardware" => serialport::FlowControl::Hardware,
        _ => panic!("Invalid string of flow control"),
    }
}

// Conversion functions
fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect::<Vec<_>>()
        .join(" ")
}

fn bytes_to_ascii(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&byte| {
            if byte >= 32 && byte <= 126 {
                (byte as char).to_string()
            } else {
                format!("\\x{:02x}", byte)
            }
        })
        .collect()
}

fn ascii_to_bytes(text: &str) -> Vec<u8> {
    text.as_bytes().to_vec()
}

fn hex_to_bytes(hex_text: &str) -> Result<Vec<u8>, String> {
    let hex_text = hex_text
        .trim()
        .replace(" ", "")
        .replace("\n", "")
        .replace("\r", "");
    if hex_text.len() % 2 != 0 {
        return Err("Hex string length must be even".to_string());
    }

    let mut bytes = Vec::new();
    for i in (0..hex_text.len()).step_by(2) {
        match u8::from_str_radix(&hex_text[i..i + 2], 16) {
            Ok(byte) => bytes.push(byte),
            Err(_) => return Err(format!("Invalid hex character at position {}", i)),
        }
    }
    Ok(bytes)
}

fn add_log(app_handle: &tauri::AppHandle, direction: &str, data: &[u8], new_group: bool) {
    let log = SerialPortLog {
        direction: direction.to_string(),
        content_hex: bytes_to_hex(data),
        content_ascii: bytes_to_ascii(data),
        timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
        new_group,
    };
    let _ = app_handle.emit("serial_port_log", &log);
}

fn start_read_thread(port_name: String, app_handle: tauri::AppHandle, active: Arc<AtomicBool>) {
    thread::spawn(move || {
        let mut buf = [0u8; 1024];
        let mut last_rx = std::time::Instant::now();

        while active.load(Ordering::Relaxed) {
            let result = {
                let mut ports = PORTS.lock().unwrap();
                match ports.get_mut(&port_name) {
                    Some(port) => port.read(&mut buf).map(|n| buf[..n].to_vec()),
                    None => break,
                }
            };

            match result {
                Ok(data) if !data.is_empty() => {
                    // 距上次收到数据超过 50ms，前端换行分组显示
                    let new_group = last_rx.elapsed().as_millis() > 50;
                    last_rx = std::time::Instant::now();
                    add_log(&app_handle, "RX", &data, new_group);
                }
                // WouldBlock / TimedOut 是正常的"暂时没数据"，短暂休眠避免空转
                Err(ref e)
                    if e.kind() == std::io::ErrorKind::TimedOut
                        || e.kind() == std::io::ErrorKind::WouldBlock =>
                {
                    thread::sleep(Duration::from_millis(1));
                }
                _ => {}
            }
        }
    });
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
    };
}

// 打开串口
#[tauri::command]
pub fn open_serial_port(
    window: tauri::Window,
    port_name: &str,
    baud_rate: u32,
) -> Result<String, String> {
    if PORTS.lock().unwrap().contains_key(port_name) {
        return Ok("Opened".to_string());
    }
    let config = SERIAL_PORT_CONFIG.lock().unwrap();
    let s = serialport::new(port_name, baud_rate)
        .data_bits(convert_to_data_bits(config.data_bits))
        .stop_bits(convert_to_stop_bits(config.stop_bits))
        .parity(convert_to_parity(config.parity.as_str()))
        .flow_control(convert_to_flow_control(config.flow_control.as_str()))
        .timeout(Duration::from_millis(1)) // 快速返回，不阻塞读取循环
        .open();
    return match s {
        Ok(port) => {
            PORTS.lock().unwrap().insert(port_name.to_string(), port);

            let active = Arc::new(AtomicBool::new(true));
            SERIAL_PORT_ACTIVE
                .lock()
                .unwrap()
                .insert(port_name.to_string(), active.clone());

            let app_handle = window.app_handle().clone();
            start_read_thread(port_name.to_string(), app_handle, active);

            Ok("Opened".to_string())
        }
        Err(e) => Err(e.description),
    };
}

// 关闭串口
#[tauri::command]
pub fn stop_serial_port(port_name: &str) {
    // 停止读取线程
    if let Some(active) = SERIAL_PORT_ACTIVE.lock().unwrap().remove(port_name) {
        active.store(false, Ordering::Relaxed);
    }

    // 关闭端口
    if PORTS.lock().unwrap().contains_key(port_name) {
        PORTS.lock().unwrap().remove(port_name);
    }
}

// 写入数据到串口
#[tauri::command]
pub fn write_to_serial_port(
    window: tauri::Window,
    port_name: &str,
    content: String,
    send_format: i32,
) -> Result<(), String> {
    // Convert content to bytes based on format
    let bytes = if send_format == 0 {
        // HEX format
        hex_to_bytes(&content)?
    } else {
        // ASCII format
        ascii_to_bytes(&content)
    };

    if let Some(port) = PORTS.lock().unwrap().get_mut(port_name) {
        match port.write(&bytes) {
            Ok(_) => {
                add_log(window.app_handle(), "TX", &bytes, false);
                Ok(())
            }
            Err(e) => Err(e.to_string()),
        }
    } else {
        Err("Serial port not open".to_string())
    }
}

// 检查串口是否已打开
#[tauri::command]
pub fn is_serial_port_open(port_name: &str) -> bool {
    PORTS.lock().unwrap().get_mut(port_name).is_some()
}
