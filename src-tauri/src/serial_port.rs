use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Debug)]
pub struct SerialPortConfig {
    data_bits: i8,
    stop_bits: i8,
    parity: String,
    flow_control: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SerialPortLog {
    pub operation: String, // "RX" or "TX"
    pub data: Vec<u8>,
    pub timestamp: u64,
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
static SERIAL_PORT_LOGS: Lazy<Mutex<VecDeque<SerialPortLog>>> =
    Lazy::new(|| Mutex::new(VecDeque::new()));
static SERIAL_PORT_ACTIVE: Lazy<Mutex<HashMap<String, Arc<AtomicBool>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

const MAX_LOGS: usize = 1000;
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

fn get_current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_millis() as u64
}

fn add_log(operation: &str, data: Vec<u8>) {
    let mut logs = SERIAL_PORT_LOGS.lock().unwrap();
    logs.push_back(SerialPortLog {
        operation: operation.to_string(),
        data,
        timestamp: get_current_timestamp(),
    });
    if logs.len() > MAX_LOGS {
        logs.pop_front();
    }
}

fn read_from_serial_port(port_name: &str) {
    let mut serial_buf: Vec<u8> = Vec::new();
    if let Some(port) = PORTS.lock().unwrap().get_mut(port_name) {
        if let Err(_) = port.read(serial_buf.as_mut_slice()) {
            return;
        }
        if !serial_buf.is_empty() {
            add_log("RX", serial_buf.clone());
        }
    }
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
pub fn open_serial_port(port_name: &str, baud_rate: u32) -> Result<String, String> {
    if PORTS.lock().unwrap().contains_key(port_name) {
        return Ok("Opened".to_string());
    }
    let config = SERIAL_PORT_CONFIG.lock().unwrap();
    let s = serialport::new(port_name, baud_rate)
        .data_bits(convert_to_data_bits(config.data_bits))
        .stop_bits(convert_to_stop_bits(config.stop_bits))
        .parity(convert_to_parity(config.parity.as_str()))
        .flow_control(convert_to_flow_control(config.flow_control.as_str()))
        .timeout(Duration::from_millis(200))
        .open();
    return match s {
        Ok(port) => {
            PORTS.lock().unwrap().insert(port_name.to_string(), port);

            // 启动后台线程，每200毫秒读取一次数据
            let active = Arc::new(AtomicBool::new(true));
            SERIAL_PORT_ACTIVE
                .lock()
                .unwrap()
                .insert(port_name.to_string(), active.clone());

            let port_name_clone = port_name.to_string();
            thread::spawn(move || {
                while active.load(Ordering::Relaxed) {
                    read_from_serial_port(&port_name_clone);
                    thread::sleep(Duration::from_millis(200));
                }
            });

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
pub fn write_to_serial_port(port_name: &str, content: Vec<u8>) {
    let result = if let Some(port) = PORTS.lock().unwrap().get_mut(port_name) {
        if let Ok(_) = port.write(&content) {
            add_log("TX", content);
        }
    };
}

// 获取串口日志
#[tauri::command]
pub fn get_serial_port_logs() -> Vec<SerialPortLog> {
    let logs = SERIAL_PORT_LOGS.lock().unwrap();
    logs.iter().cloned().collect()
}

// 检查串口是否已打开
#[tauri::command]
pub fn is_serial_port_open(port_name: &str) -> bool {
    PORTS.lock().unwrap().get_mut(port_name).is_some()
}
