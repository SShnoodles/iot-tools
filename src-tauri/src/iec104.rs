use once_cell::sync::Lazy;
use serde::Serialize;
use std::sync::{
    atomic::{AtomicU16, AtomicU64, Ordering},
    Arc, Mutex,
};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::sync::Mutex as AsyncMutex;

const SEQUENCE_MASK: u16 = 0x7fff;

#[derive(Clone)]
struct Iec104Connection {
    writer: Arc<AsyncMutex<OwnedWriteHalf>>,
    send_sequence: Arc<AtomicU16>,
    receive_sequence: Arc<AtomicU16>,
    generation: u64,
}

static IEC104_CONNECTION: Lazy<Mutex<Option<Iec104Connection>>> = Lazy::new(|| Mutex::new(None));
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Serialize)]
pub struct Iec104Point {
    pub ioa: u32,
    pub value: String,
    pub quality: String,
    pub timestamp: String,
}

#[derive(Clone, Serialize)]
pub struct Iec104Frame {
    pub direction: String,
    pub hex: String,
    pub format: String,
    pub type_id: Option<u8>,
    pub type_name: String,
    pub send_sequence: Option<u16>,
    pub receive_sequence: Option<u16>,
    pub cause: Option<u8>,
    pub common_address: Option<u16>,
    pub summary: String,
    pub points: Vec<Iec104Point>,
}

fn to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_hex(input: &str) -> Result<Vec<u8>, String> {
    let compact: String = input
        .chars()
        .filter(|character| {
            !character.is_ascii_whitespace() && *character != '-' && *character != ':'
        })
        .collect();
    if compact.is_empty() {
        return Err("APDU cannot be empty".into());
    }
    if !compact.is_ascii() {
        return Err("Hex input may only contain ASCII hexadecimal digits".into());
    }
    if compact.len() & 1 != 0 {
        return Err("Hex input must contain an even number of digits".into());
    }
    (0..compact.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&compact[index..index + 2], 16)
                .map_err(|_| format!("Invalid hex byte: {}", &compact[index..index + 2]))
        })
        .collect()
}

fn sequence_control(sequence: u16) -> [u8; 2] {
    ((sequence & SEQUENCE_MASK) << 1).to_le_bytes()
}

fn i_frame(asdu: &[u8], send_sequence: u16, receive_sequence: u16) -> Result<Vec<u8>, String> {
    let length = asdu.len() + 4;
    if !(4..=253).contains(&length) {
        return Err("ASDU is too large for an IEC104 APDU".into());
    }
    let mut frame = Vec::with_capacity(length + 2);
    frame.extend_from_slice(&[0x68, length as u8]);
    frame.extend_from_slice(&sequence_control(send_sequence));
    frame.extend_from_slice(&sequence_control(receive_sequence));
    frame.extend_from_slice(asdu);
    Ok(frame)
}

fn s_frame(receive_sequence: u16) -> Vec<u8> {
    let receive = sequence_control(receive_sequence);
    vec![0x68, 0x04, 0x01, 0x00, receive[0], receive[1]]
}

fn u_frame(control: &str) -> Result<Vec<u8>, String> {
    let byte = match control {
        "start" => 0x07,
        "stop" => 0x13,
        "test" => 0x43,
        _ => return Err(format!("Unsupported U-frame control: {control}")),
    };
    Ok(vec![0x68, 0x04, byte, 0x00, 0x00, 0x00])
}

fn type_name(type_id: u8) -> &'static str {
    match type_id {
        1 => "M_SP_NA_1",
        3 => "M_DP_NA_1",
        5 => "M_ST_NA_1",
        7 => "M_BO_NA_1",
        9 => "M_ME_NA_1",
        11 => "M_ME_NB_1",
        13 => "M_ME_NC_1",
        15 => "M_IT_NA_1",
        30 => "M_SP_TB_1",
        31 => "M_DP_TB_1",
        32 => "M_ST_TB_1",
        33 => "M_BO_TB_1",
        34 => "M_ME_TD_1",
        35 => "M_ME_TE_1",
        36 => "M_ME_TF_1",
        37 => "M_IT_TB_1",
        45 => "C_SC_NA_1",
        46 => "C_DC_NA_1",
        48 => "C_SE_NA_1",
        49 => "C_SE_NB_1",
        50 => "C_SE_NC_1",
        58 => "C_SC_TA_1",
        59 => "C_DC_TA_1",
        61 => "C_SE_TA_1",
        62 => "C_SE_TB_1",
        63 => "C_SE_TC_1",
        70 => "M_EI_NA_1",
        100 => "C_IC_NA_1",
        101 => "C_CI_NA_1",
        102 => "C_RD_NA_1",
        103 => "C_CS_NA_1",
        105 => "C_RP_NA_1",
        _ => "UNKNOWN",
    }
}

fn cause_name(cause: u8) -> &'static str {
    match cause & 0x3f {
        1 => "periodic",
        2 => "background",
        3 => "spontaneous",
        4 => "initialized",
        5 => "request",
        6 => "activation",
        7 => "activation confirmation",
        10 => "activation termination",
        20 => "interrogated by station",
        44 => "unknown type",
        45 => "unknown cause",
        46 => "unknown common address",
        47 => "unknown information address",
        _ => "other",
    }
}

fn quality(byte: u8) -> String {
    let mut flags = Vec::new();
    if byte & 0x80 != 0 {
        flags.push("IV");
    }
    if byte & 0x40 != 0 {
        flags.push("NT");
    }
    if byte & 0x20 != 0 {
        flags.push("SB");
    }
    if byte & 0x10 != 0 {
        flags.push("BL");
    }
    if byte & 0x01 != 0 {
        flags.push("OV");
    }
    if flags.is_empty() {
        "Good".into()
    } else {
        flags.join(",")
    }
}

fn counter_quality(byte: u8) -> String {
    let mut flags = Vec::new();
    if byte & 0x80 != 0 {
        flags.push("IV".to_string());
    }
    if byte & 0x40 != 0 {
        flags.push("CA".to_string());
    }
    if byte & 0x20 != 0 {
        flags.push("CY".to_string());
    }
    let sequence = byte & 0x1f;
    if sequence != 0 {
        flags.push(format!("SQ={sequence}"));
    }
    if flags.is_empty() {
        "Good".into()
    } else {
        flags.join(",")
    }
}

fn cp56_time(bytes: &[u8]) -> String {
    if bytes.len() < 7 {
        return String::new();
    }
    let milliseconds = u16::from_le_bytes([bytes[0], bytes[1]]);
    let minute = bytes[2] & 0x3f;
    let hour = bytes[3] & 0x1f;
    let day = bytes[4] & 0x1f;
    let month = bytes[5] & 0x0f;
    let year = 2000 + (bytes[6] & 0x7f) as u16;
    format!(
        "{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{:02}.{:03}",
        milliseconds / 1000,
        milliseconds % 1000
    )
}

fn object_layout(type_id: u8) -> Option<(usize, usize)> {
    match type_id {
        1 | 3 => Some((1, 0)),
        5 => Some((2, 0)),
        7 => Some((5, 0)),
        9 | 11 => Some((3, 0)),
        13 => Some((5, 0)),
        15 => Some((5, 0)),
        30 | 31 => Some((8, 7)),
        32 => Some((9, 7)),
        33 => Some((12, 7)),
        34 | 35 => Some((10, 7)),
        36 => Some((12, 7)),
        37 => Some((12, 7)),
        45 | 46 => Some((1, 0)),
        48 | 49 => Some((3, 0)),
        50 => Some((5, 0)),
        58 | 59 => Some((8, 7)),
        61 | 62 => Some((10, 7)),
        63 => Some((12, 7)),
        100 | 101 | 105 => Some((1, 0)),
        102 => Some((0, 0)),
        103 => Some((7, 7)),
        _ => None,
    }
}

fn object_value(type_id: u8, bytes: &[u8]) -> (String, String) {
    match type_id {
        1 | 30 => ((bytes[0] & 0x01 != 0).to_string(), quality(bytes[0] & 0xf0)),
        3 | 31 => ((bytes[0] & 0x03).to_string(), quality(bytes[0] & 0xf0)),
        5 | 32 => (((bytes[0] & 0x7f) as i8).to_string(), quality(bytes[1])),
        7 | 33 => (
            format!(
                "0x{:08X}",
                u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
            ),
            quality(bytes[4]),
        ),
        9 | 34 => {
            let raw = i16::from_le_bytes([bytes[0], bytes[1]]);
            (
                format!("{} ({:.6})", raw, raw as f64 / 32768.0),
                quality(bytes[2]),
            )
        }
        11 | 35 | 48 | 49 | 61 | 62 => (
            i16::from_le_bytes([bytes[0], bytes[1]]).to_string(),
            quality(bytes[2]),
        ),
        13 | 36 | 50 | 63 => (
            f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]).to_string(),
            quality(bytes[4]),
        ),
        15 | 37 => (
            i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]).to_string(),
            counter_quality(bytes[4]),
        ),
        45 | 58 => ((bytes[0] & 0x01 != 0).to_string(), String::new()),
        46 | 59 => ((bytes[0] & 0x03).to_string(), String::new()),
        100 => (format!("QOI={}", bytes[0]), String::new()),
        101 => (format!("QCC={}", bytes[0]), String::new()),
        102 => ("read".into(), String::new()),
        103 => (cp56_time(bytes), String::new()),
        105 => (format!("QRP={}", bytes[0]), String::new()),
        _ => (to_hex(bytes), String::new()),
    }
}

fn parse_points(type_id: u8, vsq: u8, payload: &[u8]) -> Vec<Iec104Point> {
    let count = (vsq & 0x7f) as usize;
    let sequential = vsq & 0x80 != 0;
    let Some((value_length, time_length)) = object_layout(type_id) else {
        return vec![];
    };
    if count == 0 {
        return vec![];
    }
    let mut points = Vec::new();
    let mut offset = 0usize;
    let mut sequential_ioa = 0u32;
    for index in 0..count {
        let ioa = if sequential && index > 0 {
            sequential_ioa + index as u32
        } else {
            if offset + 3 > payload.len() {
                break;
            }
            let address = payload[offset] as u32
                | ((payload[offset + 1] as u32) << 8)
                | ((payload[offset + 2] as u32) << 16);
            offset += 3;
            if index == 0 {
                sequential_ioa = address;
            }
            address
        };
        if offset + value_length > payload.len() {
            break;
        }
        let value_bytes = &payload[offset..offset + value_length];
        offset += value_length;
        let (value, quality) = object_value(type_id, value_bytes);
        let timestamp = if time_length > 0 {
            if value_length < time_length {
                break;
            }
            cp56_time(&value_bytes[value_length - time_length..])
        } else {
            String::new()
        };
        points.push(Iec104Point {
            ioa,
            value,
            quality,
            timestamp,
        });
    }
    points
}

fn parse_frame(bytes: &[u8], direction: &str) -> Iec104Frame {
    let mut frame = Iec104Frame {
        direction: direction.into(),
        hex: to_hex(bytes),
        format: "Invalid".into(),
        type_id: None,
        type_name: String::new(),
        send_sequence: None,
        receive_sequence: None,
        cause: None,
        common_address: None,
        summary: "Invalid APDU".into(),
        points: vec![],
    };
    if bytes.len() < 6 || bytes[0] != 0x68 || bytes[1] as usize + 2 != bytes.len() {
        return frame;
    }
    let control = &bytes[2..6];
    if control[0] & 0x01 == 0 {
        frame.format = "I".into();
        let send = u16::from_le_bytes([control[0], control[1]]) >> 1;
        let receive = u16::from_le_bytes([control[2], control[3]]) >> 1;
        frame.send_sequence = Some(send);
        frame.receive_sequence = Some(receive);
        if bytes.len() < 12 {
            frame.summary = format!("I NS={send} NR={receive} | ASDU truncated");
            return frame;
        }
        let asdu = &bytes[6..];
        let type_id = asdu[0];
        let vsq = asdu[1];
        let cause = asdu[2] & 0x3f;
        let common_address = u16::from_le_bytes([asdu[4], asdu[5]]);
        let name = type_name(type_id);
        frame.type_id = Some(type_id);
        frame.type_name = name.into();
        frame.cause = Some(cause);
        frame.common_address = Some(common_address);
        frame.points = parse_points(type_id, vsq, &asdu[6..]);
        frame.summary = format!(
            "I NS={send} NR={receive} | {name} ({type_id}) | COT={} {} | CA={common_address} | NUM={}",
            cause,
            cause_name(cause),
            vsq & 0x7f
        );
    } else if control[0] & 0x03 == 0x01 {
        let receive = u16::from_le_bytes([control[2], control[3]]) >> 1;
        frame.format = "S".into();
        frame.receive_sequence = Some(receive);
        frame.summary = format!("S NR={receive}");
    } else {
        frame.format = "U".into();
        frame.type_name = match control[0] {
            0x07 => "STARTDT_ACT",
            0x0b => "STARTDT_CON",
            0x13 => "STOPDT_ACT",
            0x23 => "STOPDT_CON",
            0x43 => "TESTFR_ACT",
            0x83 => "TESTFR_CON",
            _ => "UNKNOWN_U",
        }
        .into();
        frame.summary = format!("U {}", frame.type_name);
    }
    frame
}

fn current_connection() -> Result<Iec104Connection, String> {
    IEC104_CONNECTION
        .lock()
        .map_err(|error| error.to_string())?
        .clone()
        .ok_or_else(|| "Not connected".into())
}

fn clear_connection_if_current(connection: &Iec104Connection) {
    if let Ok(mut state) = IEC104_CONNECTION.lock() {
        if state.as_ref().map(|item| item.generation) == Some(connection.generation) {
            *state = None;
        }
    }
}

async fn write_apdu(connection: &Iec104Connection, bytes: &[u8]) -> Result<(), String> {
    connection
        .writer
        .lock()
        .await
        .write_all(bytes)
        .await
        .map_err(|error| error.to_string())
}

async fn write_command_apdu(connection: &Iec104Connection, bytes: &[u8]) -> Result<(), String> {
    let result = write_apdu(connection, bytes).await;
    if result.is_err() {
        clear_connection_if_current(connection);
    }
    result
}

async fn send_i(asdu: &[u8]) -> Result<Iec104Frame, String> {
    let connection = current_connection()?;
    let mut writer = connection.writer.lock().await;
    let send = connection
        .send_sequence
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
            Some((value + 1) & SEQUENCE_MASK)
        })
        .unwrap_or(0);
    let receive = connection.receive_sequence.load(Ordering::SeqCst) & SEQUENCE_MASK;
    let bytes = i_frame(asdu, send, receive)?;
    let result = writer
        .write_all(&bytes)
        .await
        .map_err(|error| error.to_string());
    drop(writer);
    if result.is_err() {
        clear_connection_if_current(&connection);
    }
    result?;
    Ok(parse_frame(&bytes, "tx"))
}

async fn reader_loop(app: AppHandle, mut reader: OwnedReadHalf, connection: Iec104Connection) {
    let mut pending = Vec::<u8>::new();
    let mut chunk = [0u8; 1024];
    let reason = 'connection: loop {
        match reader.read(&mut chunk).await {
            Ok(0) => break "Remote host closed the connection".to_string(),
            Ok(count) => {
                pending.extend_from_slice(&chunk[..count]);
                loop {
                    let Some(start) = pending.iter().position(|byte| *byte == 0x68) else {
                        pending.clear();
                        break;
                    };
                    if start > 0 {
                        pending.drain(..start);
                    }
                    if pending.len() < 2 {
                        break;
                    }
                    let total = pending[1] as usize + 2;
                    if total < 6 {
                        pending.remove(0);
                        continue;
                    }
                    if pending.len() < total {
                        break;
                    }
                    let bytes: Vec<u8> = pending.drain(..total).collect();
                    let frame = parse_frame(&bytes, "rx");
                    app.emit("iec104_frame", frame.clone()).ok();

                    if frame.format == "I" {
                        if let Some(send_sequence) = frame.send_sequence {
                            let receive = (send_sequence + 1) & SEQUENCE_MASK;
                            connection.receive_sequence.store(receive, Ordering::SeqCst);
                            let acknowledgement = s_frame(receive);
                            match write_apdu(&connection, &acknowledgement).await {
                                Ok(()) => {
                                    app.emit("iec104_frame", parse_frame(&acknowledgement, "tx"))
                                        .ok();
                                }
                                Err(error) => {
                                    break 'connection format!("Failed to send S-frame: {error}");
                                }
                            }
                        }
                    } else {
                        let confirmation_control = match frame.type_name.as_str() {
                            "STARTDT_ACT" => Some(0x0b),
                            "STOPDT_ACT" => Some(0x23),
                            "TESTFR_ACT" => Some(0x83),
                            _ => None,
                        };
                        if let Some(control) = confirmation_control {
                            let confirmation = vec![0x68, 0x04, control, 0x00, 0x00, 0x00];
                            match write_apdu(&connection, &confirmation).await {
                                Ok(()) => {
                                    app.emit("iec104_frame", parse_frame(&confirmation, "tx"))
                                        .ok();
                                }
                                Err(error) => {
                                    break 'connection format!(
                                        "Failed to send U-frame confirmation: {error}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
            Err(error) => break error.to_string(),
        }
    };

    let should_emit = if let Ok(mut state) = IEC104_CONNECTION.lock() {
        if state.as_ref().map(|item| item.generation) == Some(connection.generation) {
            *state = None;
            true
        } else {
            false
        }
    } else {
        false
    };
    if should_emit {
        app.emit("iec104_disconnected", reason).ok();
    }
}

#[tauri::command]
pub async fn iec104_connect(
    app: AppHandle,
    host: String,
    port: u16,
    timeout_ms: u64,
) -> Result<(), String> {
    if host.trim().is_empty() {
        return Err("Host cannot be empty".into());
    }
    iec104_disconnect().await?;
    let stream = tokio::time::timeout(
        Duration::from_millis(timeout_ms.max(100)),
        TcpStream::connect((host.trim(), port)),
    )
    .await
    .map_err(|_| format!("Connection timed out after {timeout_ms}ms"))?
    .map_err(|error| error.to_string())?;
    stream
        .set_nodelay(true)
        .map_err(|error| error.to_string())?;
    let (reader, writer) = stream.into_split();
    let connection = Iec104Connection {
        writer: Arc::new(AsyncMutex::new(writer)),
        send_sequence: Arc::new(AtomicU16::new(0)),
        receive_sequence: Arc::new(AtomicU16::new(0)),
        generation: NEXT_GENERATION.fetch_add(1, Ordering::SeqCst),
    };
    *IEC104_CONNECTION
        .lock()
        .map_err(|error| error.to_string())? = Some(connection.clone());
    tauri::async_runtime::spawn(reader_loop(app, reader, connection));
    Ok(())
}

#[tauri::command]
pub async fn iec104_disconnect() -> Result<(), String> {
    let connection = IEC104_CONNECTION
        .lock()
        .map_err(|error| error.to_string())?
        .take();
    if let Some(connection) = connection {
        connection.writer.lock().await.shutdown().await.ok();
    }
    Ok(())
}

#[tauri::command]
pub fn iec104_is_connected() -> bool {
    IEC104_CONNECTION
        .lock()
        .map(|state| state.is_some())
        .unwrap_or(false)
}

#[tauri::command]
pub async fn iec104_send_control(control: String) -> Result<Iec104Frame, String> {
    let connection = current_connection()?;
    let bytes = u_frame(&control)?;
    write_command_apdu(&connection, &bytes).await?;
    Ok(parse_frame(&bytes, "tx"))
}

#[tauri::command]
pub async fn iec104_general_interrogation(common_address: u16) -> Result<Iec104Frame, String> {
    let mut asdu = vec![100, 1, 6, 0];
    asdu.extend_from_slice(&common_address.to_le_bytes());
    asdu.extend_from_slice(&[0, 0, 0, 20]);
    send_i(&asdu).await
}

#[tauri::command]
pub async fn iec104_clock_sync(
    common_address: u16,
    timestamp_ms: i64,
) -> Result<Iec104Frame, String> {
    let datetime = chrono::DateTime::from_timestamp_millis(timestamp_ms)
        .ok_or_else(|| "Invalid timestamp".to_string())?
        .with_timezone(&chrono::Local);
    use chrono::{Datelike, Timelike};
    let milliseconds = (datetime.second() * 1000 + datetime.timestamp_subsec_millis()) as u16;
    let cp56 = [
        milliseconds.to_le_bytes()[0],
        milliseconds.to_le_bytes()[1],
        datetime.minute() as u8,
        datetime.hour() as u8,
        datetime.day() as u8 | ((datetime.weekday().number_from_monday() as u8) << 5),
        datetime.month() as u8,
        (datetime.year() % 100) as u8,
    ];
    let mut asdu = vec![103, 1, 6, 0];
    asdu.extend_from_slice(&common_address.to_le_bytes());
    asdu.extend_from_slice(&[0, 0, 0]);
    asdu.extend_from_slice(&cp56);
    send_i(&asdu).await
}

#[tauri::command]
pub async fn iec104_send_raw(hex: String) -> Result<Iec104Frame, String> {
    let bytes = parse_hex(&hex)?;
    if bytes.len() < 6 || bytes[0] != 0x68 {
        return Err("A raw APDU must start with 68 and contain at least 6 bytes".into());
    }
    if bytes[1] as usize + 2 != bytes.len() {
        return Err(format!(
            "APDU length mismatch: length field says {}, actual payload is {}",
            bytes[1],
            bytes.len() - 2
        ));
    }
    let connection = current_connection()?;
    write_command_apdu(&connection, &bytes).await?;
    Ok(parse_frame(&bytes, "tx"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_and_parses_general_interrogation() {
        let mut asdu = vec![100, 1, 6, 0];
        asdu.extend_from_slice(&1u16.to_le_bytes());
        asdu.extend_from_slice(&[0, 0, 0, 20]);
        let bytes = i_frame(&asdu, 3, 7).unwrap();
        let parsed = parse_frame(&bytes, "tx");
        assert_eq!(parsed.format, "I");
        assert_eq!(parsed.send_sequence, Some(3));
        assert_eq!(parsed.receive_sequence, Some(7));
        assert_eq!(parsed.type_name, "C_IC_NA_1");
        assert_eq!(parsed.common_address, Some(1));
        assert_eq!(parsed.points[0].value, "QOI=20");
    }

    #[test]
    fn parses_single_point_information() {
        let asdu = [1, 1, 3, 0, 1, 0, 10, 0, 0, 1];
        let bytes = i_frame(&asdu, 0, 0).unwrap();
        let parsed = parse_frame(&bytes, "rx");
        assert_eq!(parsed.points[0].ioa, 10);
        assert_eq!(parsed.points[0].value, "true");
        assert_eq!(parsed.points[0].quality, "Good");
    }

    #[test]
    fn parses_sequential_information_objects() {
        let asdu = [1, 0x82, 20, 0, 1, 0, 10, 0, 0, 1, 0];
        let bytes = i_frame(&asdu, 0, 0).unwrap();
        let parsed = parse_frame(&bytes, "rx");
        assert_eq!(parsed.points.len(), 2);
        assert_eq!(parsed.points[0].ioa, 10);
        assert_eq!(parsed.points[0].value, "true");
        assert_eq!(parsed.points[1].ioa, 11);
        assert_eq!(parsed.points[1].value, "false");
    }

    #[test]
    fn rejects_bad_hex() {
        assert!(parse_hex("68 0G").is_err());
        assert!(parse_hex("680").is_err());
        assert!(parse_hex("测试").is_err());
    }
}
