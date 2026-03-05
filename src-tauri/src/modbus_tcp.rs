use crate::display::DisplayFormat;
use once_cell::sync::Lazy;
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::Mutex;
use tokio_modbus::prelude::*;

// 全局连接，同一时间只维护一个 Modbus TCP 连接
static MODBUS_CTX: Lazy<Mutex<Option<sync::Context>>> = Lazy::new(|| Mutex::new(None));

#[derive(Serialize)]
pub struct ModbusResponse {
    pub request_hex: String,
    pub response_hex: String,
    pub values: Vec<u16>,
    pub exception_code: Option<u8>,
    pub display_value: String,
}

// 字节数组 → 空格分隔的十六进制字符串，如 "03 00 00 00 0A"
fn to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

// u16 数组（大端序）→ 十六进制字符串
fn values_to_hex(values: &[u16]) -> String {
    let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_be_bytes()).collect();
    to_hex(&bytes)
}

// 显示格式字符串 → DisplayFormat 枚举（未知时默认 Unsigned）
fn parse_display_format(s: &str) -> DisplayFormat {
    match s {
        "Signed" => DisplayFormat::Signed,
        "Hex" => DisplayFormat::Hex,
        "Binary" => DisplayFormat::Binary,
        "Long" => DisplayFormat::Long,
        "LongInverse" => DisplayFormat::LongInverse,
        "Float" => DisplayFormat::Float,
        "FloatInverse" => DisplayFormat::FloatInverse,
        "Double" => DisplayFormat::Double,
        "DoubleInverse" => DisplayFormat::DoubleInverse,
        _ => DisplayFormat::Unsigned,
    }
}

// ── Tauri 命令 ────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn modbus_tcp_connect(host: String, port: u16) -> Result<(), String> {
    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .map_err(|e: std::net::AddrParseError| e.to_string())?;
    let ctx = sync::tcp::connect(addr).map_err(|e| e.to_string())?;
    *MODBUS_CTX.lock().map_err(|e| e.to_string())? = Some(ctx);
    Ok(())
}

#[tauri::command]
pub fn modbus_tcp_disconnect() {
    if let Ok(mut ctx) = MODBUS_CTX.lock() {
        *ctx = None;
    }
}

#[tauri::command]
pub fn modbus_tcp_is_connected() -> bool {
    MODBUS_CTX.lock().map(|c| c.is_some()).unwrap_or(false)
}

#[tauri::command]
pub fn modbus_tcp_send(
    unit_id: u8,
    function_code: u8,
    address: u16,
    quantity: u16,
    values: Vec<u16>,
    display_format: String,
) -> Result<ModbusResponse, String> {
    // 构造请求 PDU 字节用于日志显示（功能码 + 地址 + 数量/写入值）
    let mut req = vec![function_code];
    req.extend_from_slice(&address.to_be_bytes());
    match function_code {
        1 | 2 | 3 | 4 => req.extend_from_slice(&quantity.to_be_bytes()),
        5 | 6 => req.extend_from_slice(&values.first().copied().unwrap_or(0).to_be_bytes()),
        _ => {}
    }
    let request_hex = to_hex(&req);

    // 执行 Modbus 操作（持有锁期间完成，之后立即释放）
    let result = {
        let mut guard = MODBUS_CTX.lock().map_err(|e| e.to_string())?;
        let ctx = guard.as_mut().ok_or("未连接")?;
        ctx.set_slave(Slave(unit_id));
        match function_code {
            1 => ctx
                .read_coils(address, quantity)
                .map(|r| r.map(|v| v.into_iter().map(u16::from).collect::<Vec<_>>())),
            2 => ctx
                .read_discrete_inputs(address, quantity)
                .map(|r| r.map(|v| v.into_iter().map(u16::from).collect::<Vec<_>>())),
            3 => ctx.read_holding_registers(address, quantity),
            4 => ctx.read_input_registers(address, quantity),
            5 => ctx
                .write_single_coil(address, values.first().copied().unwrap_or(0) != 0)
                .map(|r| r.map(|_| vec![])),
            6 => ctx
                .write_single_register(address, values.first().copied().unwrap_or(0))
                .map(|r| r.map(|_| vec![])),
            _ => return Err(format!("不支持的功能码: {function_code}")),
        }
    };

    let fmt = parse_display_format(&display_format);

    match result {
        Ok(Ok(vals)) => Ok(ModbusResponse {
            request_hex,
            response_hex: values_to_hex(&vals),
            display_value: fmt.format(&vals),
            values: vals,
            exception_code: None,
        }),
        Ok(Err(exc)) => {
            let code = u8::from(exc);
            Ok(ModbusResponse {
                request_hex,
                response_hex: format!("Exception {code:02X}"),
                values: vec![],
                exception_code: Some(code),
                display_value: String::new(),
            })
        }
        Err(e) => {
            // I/O 错误：连接已断开，清除连接状态
            if let Ok(mut conn) = MODBUS_CTX.lock() {
                *conn = None;
            }
            Err(e.to_string())
        }
    }
}
