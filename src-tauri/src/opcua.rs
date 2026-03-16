use once_cell::sync::Lazy;
use opcua::client::{ClientBuilder, IdentityToken, Session};
use opcua::types::{
    AttributeId, DataValue, NodeId, ReadValueId, StatusCode, TimestampsToReturn, Variant,
    WriteValue,
};
use serde::Serialize;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

#[derive(Serialize)]
pub struct NodeValue {
    pub value: String,
    pub data_type: String,
    pub status: String,
    pub source_timestamp: String,
}

static OPCUA_SESSION: Lazy<Mutex<Option<Arc<Session>>>> = Lazy::new(|| Mutex::new(None));
static OPCUA_CONNECTED: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));

#[tauri::command]
pub fn opcua_connect(
    endpoint_url: String,
    username: Option<String>,
    password: Option<String>,
) -> Result<(), String> {
    {
        let mut s = OPCUA_SESSION.lock().unwrap();
        *s = None;
        *OPCUA_CONNECTED.lock().unwrap() = false;
    }

    let identity = match username {
        Some(user) => IdentityToken::UserName(user, password.unwrap_or_default()),
        None => IdentityToken::Anonymous,
    };

    let session = tauri::async_runtime::block_on(async {
        let mut client = ClientBuilder::new()
            .application_name("iot-tools")
            .application_uri("urn:iot-tools")
            .trust_server_certs(true)
            .create_sample_keypair(true)
            .client()
            .map_err(|e| e.join(", "))?;

        let (session, event_loop) = client
            .connect_to_matching_endpoint(endpoint_url.as_str(), identity)
            .await
            .map_err(|e: StatusCode| e.to_string())?;

        tauri::async_runtime::spawn(async move {
            event_loop.run().await;
            *OPCUA_CONNECTED.lock().unwrap() = false;
            *OPCUA_SESSION.lock().unwrap() = None;
        });

        let connected = session.wait_for_connection().await;
        if !connected {
            return Err("Failed to establish session with server".to_string());
        }

        Ok::<Arc<Session>, String>(session)
    })?;

    *OPCUA_SESSION.lock().unwrap() = Some(session);
    *OPCUA_CONNECTED.lock().unwrap() = true;

    Ok(())
}

#[tauri::command]
pub fn opcua_disconnect() -> Result<(), String> {
    let session = OPCUA_SESSION.lock().unwrap().take();
    *OPCUA_CONNECTED.lock().unwrap() = false;
    if let Some(s) = session {
        tauri::async_runtime::block_on(async move {
            s.disconnect().await.ok();
        });
    }
    Ok(())
}

#[tauri::command]
pub fn opcua_is_connected() -> bool {
    *OPCUA_CONNECTED.lock().unwrap()
}

#[tauri::command]
pub fn opcua_read_node(node_id: String, timeout_ms: u64) -> Result<NodeValue, String> {
    let session = OPCUA_SESSION.lock().unwrap().clone();
    let session = session.ok_or("Not connected")?;

    let nid = NodeId::from_str(&node_id).map_err(|e| e.to_string())?;

    tauri::async_runtime::block_on(async move {
        let duration = std::time::Duration::from_millis(timeout_ms);
        let results = tokio::time::timeout(duration, session.read(
                &[ReadValueId {
                    node_id: nid,
                    attribute_id: AttributeId::Value as u32,
                    ..Default::default()
                }],
                TimestampsToReturn::Both,
                0.0,
            ))
            .await
            .map_err(|_| format!("Read timed out after {timeout_ms}ms"))?
            .map_err(|e: StatusCode| e.to_string())?;

        let dv = results.into_iter().next().ok_or("No result returned")?;

        let status = match dv.status {
            Some(s) => {
                if s.is_good() {
                    "Good".to_string()
                } else if s.is_uncertain() {
                    format!("Uncertain ({s})")
                } else {
                    format!("Bad ({s})")
                }
            }
            None => "Good".to_string(),
        };

        let source_timestamp = dv
            .source_timestamp
            .map(|t| t.to_string())
            .unwrap_or_default();

        match dv.value {
            Some(v) => Ok(NodeValue {
                data_type: variant_type_name(&v).to_string(),
                value: variant_to_string(v),
                status,
                source_timestamp,
            }),
            None => Err(format!("No value, status: {status}")),
        }
    })
}

#[tauri::command]
pub fn opcua_write_node(node_id: String, value: String) -> Result<(), String> {
    let session = OPCUA_SESSION.lock().unwrap().clone();
    let session = session.ok_or("Not connected")?;

    let nid = NodeId::from_str(&node_id).map_err(|e| e.to_string())?;

    tauri::async_runtime::block_on(async move {
        let results = session
            .write(&[WriteValue {
                node_id: nid,
                attribute_id: AttributeId::Value as u32,
                value: DataValue {
                    value: Some(Variant::String(value.into())),
                    ..Default::default()
                },
                ..Default::default()
            }])
            .await
            .map_err(|e: StatusCode| e.to_string())?;

        match results.into_iter().next() {
            Some(s) if s.is_good() => Ok(()),
            Some(s) => Err(format!("Write failed: {s}")),
            None => Err("No result returned".into()),
        }
    })
}

fn variant_type_name(v: &Variant) -> &'static str {
    match v {
        Variant::Boolean(_) => "Boolean",
        Variant::SByte(_) => "SByte",
        Variant::Byte(_) => "Byte",
        Variant::Int16(_) => "Int16",
        Variant::UInt16(_) => "UInt16",
        Variant::Int32(_) => "Int32",
        Variant::UInt32(_) => "UInt32",
        Variant::Int64(_) => "Int64",
        Variant::UInt64(_) => "UInt64",
        Variant::Float(_) => "Float",
        Variant::Double(_) => "Double",
        Variant::String(_) => "String",
        Variant::DateTime(_) => "DateTime",
        Variant::Guid(_) => "Guid",
        Variant::ByteString(_) => "ByteString",
        Variant::NodeId(_) => "NodeId",
        Variant::Array(_) => "Array",
        _ => "Unknown",
    }
}

fn variant_to_string(v: Variant) -> String {
    match v {
        Variant::Boolean(b) => b.to_string(),
        Variant::SByte(n) => n.to_string(),
        Variant::Byte(n) => n.to_string(),
        Variant::Int16(n) => n.to_string(),
        Variant::UInt16(n) => n.to_string(),
        Variant::Int32(n) => n.to_string(),
        Variant::UInt32(n) => n.to_string(),
        Variant::Int64(n) => n.to_string(),
        Variant::UInt64(n) => n.to_string(),
        Variant::Float(f) => f.to_string(),
        Variant::Double(d) => d.to_string(),
        Variant::String(s) => s.to_string(),
        Variant::DateTime(dt) => dt.to_string(),
        other => format!("{other:?}"),
    }
}
