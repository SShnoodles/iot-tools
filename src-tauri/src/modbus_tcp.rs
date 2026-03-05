use crate::display::DisplayFormat;
use once_cell::sync::Lazy;
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::{
    atomic::{AtomicU16, Ordering},
    Mutex,
};
use tokio_modbus::prelude::*;

static MODBUS_CTX: Lazy<Mutex<Option<sync::Context>>> = Lazy::new(|| Mutex::new(None));
static TID: AtomicU16 = AtomicU16::new(0);

#[derive(Serialize)]
pub struct ModbusResponse {
    pub request_hex: String,
    pub response_hex: String,
    pub values: Vec<u16>,
    pub exception_code: Option<u8>,
    pub display_value: String,
}

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

// ── hex helpers ──────────────────────────────────────────────────────────────

fn to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Build MBAP + PDU request frame (for display; actual send is via tokio-modbus).
fn request_frame(
    tid: u16,
    unit_id: u8,
    fc: u8,
    address: u16,
    quantity: u16,
    values: &[u16],
) -> Vec<u8> {
    let mut pdu = vec![fc];
    match fc {
        1 | 2 | 3 | 4 => {
            pdu.extend_from_slice(&address.to_be_bytes());
            pdu.extend_from_slice(&quantity.to_be_bytes());
        }
        5 => {
            let coil_val: u16 = if values.first().copied().unwrap_or(0) != 0 {
                0xFF00
            } else {
                0
            };
            pdu.extend_from_slice(&address.to_be_bytes());
            pdu.extend_from_slice(&coil_val.to_be_bytes());
        }
        6 => {
            pdu.extend_from_slice(&address.to_be_bytes());
            pdu.extend_from_slice(&values.first().copied().unwrap_or(0).to_be_bytes());
        }
        _ => {}
    }
    mbap_wrap(tid, unit_id, pdu)
}

/// Reconstruct response frame from parsed values (for display).
fn response_frame(
    tid: u16,
    unit_id: u8,
    fc: u8,
    address: u16,
    values: &[u16],
    exc: Option<u8>,
) -> Vec<u8> {
    let pdu = if let Some(code) = exc {
        vec![fc | 0x80, code]
    } else {
        match fc {
            1 | 2 => {
                let byte_count = (values.len() + 7) / 8;
                let mut coil_bytes = vec![0u8; byte_count];
                for (i, &v) in values.iter().enumerate() {
                    if v != 0 {
                        coil_bytes[i / 8] |= 1 << (i % 8);
                    }
                }
                let mut p = vec![fc, byte_count as u8];
                p.extend_from_slice(&coil_bytes);
                p
            }
            3 | 4 => {
                let mut p = vec![fc, (values.len() * 2) as u8];
                for &v in values {
                    p.extend_from_slice(&v.to_be_bytes());
                }
                p
            }
            5 => {
                let coil_val: u16 = if values.first().copied().unwrap_or(0) != 0 {
                    0xFF00
                } else {
                    0
                };
                let mut p = vec![fc];
                p.extend_from_slice(&address.to_be_bytes());
                p.extend_from_slice(&coil_val.to_be_bytes());
                p
            }
            6 => {
                let mut p = vec![fc];
                p.extend_from_slice(&address.to_be_bytes());
                p.extend_from_slice(&values.first().copied().unwrap_or(0).to_be_bytes());
                p
            }
            _ => vec![fc],
        }
    };
    mbap_wrap(tid, unit_id, pdu)
}

fn mbap_wrap(tid: u16, unit_id: u8, pdu: Vec<u8>) -> Vec<u8> {
    let mbap_len = (1u16 + pdu.len() as u16).to_be_bytes();
    let mut frame = Vec::with_capacity(7 + pdu.len());
    frame.extend_from_slice(&tid.to_be_bytes());
    frame.extend_from_slice(&[0x00, 0x00]); // Protocol ID
    frame.extend_from_slice(&mbap_len);
    frame.push(unit_id);
    frame.extend_from_slice(&pdu);
    frame
}

// ── Tauri commands ───────────────────────────────────────────────────────────

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
    let tid = TID.fetch_add(1, Ordering::Relaxed);
    let request_hex = to_hex(&request_frame(
        tid,
        unit_id,
        function_code,
        address,
        quantity,
        &values,
    ));

    // tokio-modbus 0.14 returns Result<Result<T, Exception>, io::Error>:
    //   outer Err  → I/O / transport error (connection broken)
    //   inner Err  → Modbus exception response (connection still valid)
    //   inner Ok   → success
    let io_result = {
        let mut guard = MODBUS_CTX.lock().map_err(|e| e.to_string())?;
        let ctx = guard.as_mut().ok_or_else(|| "未连接".to_string())?;
        ctx.set_slave(Slave(unit_id));

        match function_code {
            1 => ctx
                .read_coils(address, quantity)
                .map(|r| r.map(|coils| coils.into_iter().map(u16::from).collect::<Vec<_>>())),
            2 => ctx
                .read_discrete_inputs(address, quantity)
                .map(|r| r.map(|coils| coils.into_iter().map(u16::from).collect::<Vec<_>>())),
            3 => ctx.read_holding_registers(address, quantity),
            4 => ctx.read_input_registers(address, quantity),
            5 => {
                let coil = values.first().copied().unwrap_or(0) != 0;
                ctx.write_single_coil(address, coil)
                    .map(|r| r.map(|_| vec![]))
            }
            6 => {
                let val = values.first().copied().unwrap_or(0);
                ctx.write_single_register(address, val)
                    .map(|r| r.map(|_| vec![]))
            }
            _ => return Err(format!("不支持的功能码: {function_code}")),
        }
    }; // MutexGuard released here

    let fmt = parse_display_format(&display_format);

    match io_result {
        Ok(Ok(vals)) => {
            let response_hex = to_hex(&response_frame(
                tid,
                unit_id,
                function_code,
                address,
                &vals,
                None,
            ));
            let display_value = fmt.format(&vals);
            Ok(ModbusResponse {
                request_hex,
                response_hex,
                values: vals,
                exception_code: None,
                display_value,
            })
        }
        Ok(Err(exc)) => {
            // Modbus exception: connection remains valid
            let code = u8::from(exc);
            let response_hex = to_hex(&response_frame(
                tid,
                unit_id,
                function_code,
                address,
                &[],
                Some(code),
            ));
            Ok(ModbusResponse {
                request_hex,
                response_hex,
                values: vec![],
                exception_code: Some(code),
                display_value: String::new(),
            })
        }
        Err(e) => {
            // I/O error: connection is broken, clear it
            if let Ok(mut conn) = MODBUS_CTX.lock() {
                *conn = None;
            }
            Err(e.to_string())
        }
    }
}
