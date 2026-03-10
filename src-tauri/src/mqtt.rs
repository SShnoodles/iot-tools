use once_cell::sync::Lazy;
use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};

static MQTT_CLIENT: Lazy<Mutex<Option<AsyncClient>>> = Lazy::new(|| Mutex::new(None));
static MQTT_CONNECTED: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));
static MQTT_DISCONNECTING: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));

#[derive(Serialize, Clone)]
pub struct MqttMessage {
    pub topic: String,
    pub payload: String,
    pub qos: u8,
    pub retain: bool,
}

fn u8_to_qos(qos: u8) -> QoS {
    match qos {
        1 => QoS::AtLeastOnce,
        2 => QoS::ExactlyOnce,
        _ => QoS::AtMostOnce,
    }
}

#[tauri::command]
pub async fn mqtt_connect(
    app: AppHandle,
    host: String,
    port: u16,
    client_id: String,
    username: Option<String>,
    password: Option<String>,
) -> Result<(), String> {
    // Clean up any existing connection
    {
        let mut c = MQTT_CLIENT.lock().unwrap();
        let mut connected = MQTT_CONNECTED.lock().unwrap();
        *c = None;
        *connected = false;
    }

    let mut options = MqttOptions::new(client_id, &host, port);
    options.set_keep_alive(std::time::Duration::from_secs(30));
    options.set_clean_session(true);
    if let Some(user) = username {
        options.set_credentials(user, password.unwrap_or_default());
    }

    let (client, mut eventloop) = AsyncClient::new(options, 64);
    *MQTT_CLIENT.lock().unwrap() = Some(client);

    tauri::async_runtime::spawn(async move {
        loop {
            match eventloop.poll().await {
                Ok(Event::Incoming(Packet::ConnAck(_))) => {
                    *MQTT_CONNECTED.lock().unwrap() = true;
                    app.emit("mqtt_connected", ()).ok();
                }
                Ok(Event::Incoming(Packet::Publish(p))) => {
                    let payload = String::from_utf8_lossy(&p.payload).to_string();
                    app.emit(
                        "mqtt_message",
                        MqttMessage {
                            topic: p.topic.clone(),
                            payload,
                            qos: p.qos as u8,
                            retain: p.retain,
                        },
                    )
                    .ok();
                }
                Ok(_) => {}
                Err(e) => {
                    let disconnecting = {
                        let mut flag = MQTT_DISCONNECTING.lock().unwrap();
                        let was = *flag;
                        *flag = false;
                        was
                    };
                    *MQTT_CONNECTED.lock().unwrap() = false;
                    *MQTT_CLIENT.lock().unwrap() = None;
                    if disconnecting {
                        app.emit("mqtt_disconnected", "").ok();
                    } else {
                        app.emit("mqtt_disconnected", e.to_string()).ok();
                    }
                    break;
                }
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn mqtt_disconnect() -> Result<(), String> {
    let client = MQTT_CLIENT.lock().unwrap().clone();
    *MQTT_DISCONNECTING.lock().unwrap() = true;
    *MQTT_CONNECTED.lock().unwrap() = false;
    *MQTT_CLIENT.lock().unwrap() = None;
    if let Some(c) = client {
        c.disconnect().await.ok();
    }
    Ok(())
}

#[tauri::command]
pub fn mqtt_is_connected() -> bool {
    *MQTT_CONNECTED.lock().unwrap()
}

#[tauri::command]
pub async fn mqtt_subscribe(topic: String, qos: u8) -> Result<(), String> {
    let client = MQTT_CLIENT.lock().unwrap().clone();
    match client {
        Some(c) => c
            .subscribe(topic, u8_to_qos(qos))
            .await
            .map_err(|e| e.to_string()),
        None => Err("Not connected".into()),
    }
}

#[tauri::command]
pub async fn mqtt_unsubscribe(topic: String) -> Result<(), String> {
    let client = MQTT_CLIENT.lock().unwrap().clone();
    match client {
        Some(c) => c.unsubscribe(topic).await.map_err(|e| e.to_string()),
        None => Err("Not connected".into()),
    }
}

#[tauri::command]
pub async fn mqtt_publish(
    topic: String,
    payload: String,
    qos: u8,
    retain: bool,
) -> Result<(), String> {
    let client = MQTT_CLIENT.lock().unwrap().clone();
    match client {
        Some(c) => c
            .publish(topic, u8_to_qos(qos), retain, payload.into_bytes())
            .await
            .map_err(|e| e.to_string()),
        None => Err("Not connected".into()),
    }
}
