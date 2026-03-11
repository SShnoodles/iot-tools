use once_cell::sync::Lazy;
use rumqttc::{AsyncClient as V311Client, Event as V311Event, MqttOptions as V311Options, Packet as V311Packet, QoS};
use rumqttc::v5::{AsyncClient as V5Client, Event as V5Event, MqttOptions as V5Options};
use rumqttc::v5::mqttbytes::v5::Packet as V5Packet;
use rumqttc::v5::mqttbytes::QoS as V5QoS;
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};

enum MqttClientEnum {
    V311(V311Client),
    V5(V5Client),
}

static MQTT_CLIENT: Lazy<Mutex<Option<MqttClientEnum>>> = Lazy::new(|| Mutex::new(None));
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

fn u8_to_v5_qos(qos: u8) -> V5QoS {
    match qos {
        1 => V5QoS::AtLeastOnce,
        2 => V5QoS::ExactlyOnce,
        _ => V5QoS::AtMostOnce,
    }
}

fn check_and_clear_disconnecting() -> bool {
    let mut flag = MQTT_DISCONNECTING.lock().unwrap();
    let was = *flag;
    *flag = false;
    was
}

#[tauri::command]
pub async fn mqtt_connect(
    app: AppHandle,
    host: String,
    port: u16,
    client_id: String,
    username: Option<String>,
    password: Option<String>,
    protocol: String,
) -> Result<(), String> {
    // Clean up any existing connection
    {
        let mut c = MQTT_CLIENT.lock().unwrap();
        let mut connected = MQTT_CONNECTED.lock().unwrap();
        *c = None;
        *connected = false;
    }

    if protocol == "v5" {
        let mut options = V5Options::new(client_id, &host, port);
        options.set_keep_alive(std::time::Duration::from_secs(30));
        options.set_clean_start(true);
        if let Some(user) = username {
            options.set_credentials(user, password.unwrap_or_default());
        }

        let (client, mut eventloop) = V5Client::new(options, 64);
        *MQTT_CLIENT.lock().unwrap() = Some(MqttClientEnum::V5(client));

        tauri::async_runtime::spawn(async move {
            loop {
                match eventloop.poll().await {
                    Ok(V5Event::Incoming(V5Packet::ConnAck(_))) => {
                        *MQTT_CONNECTED.lock().unwrap() = true;
                        app.emit("mqtt_connected", ()).ok();
                    }
                    Ok(V5Event::Incoming(V5Packet::Publish(p))) => {
                        let topic = String::from_utf8_lossy(&p.topic).to_string();
                        let payload = String::from_utf8_lossy(&p.payload).to_string();
                        app.emit(
                            "mqtt_message",
                            MqttMessage {
                                topic,
                                payload,
                                qos: p.qos as u8,
                                retain: p.retain,
                            },
                        )
                        .ok();
                    }
                    Ok(_) => {}
                    Err(e) => {
                        let disconnecting = check_and_clear_disconnecting();
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
    } else {
        let mut options = V311Options::new(client_id, &host, port);
        options.set_keep_alive(std::time::Duration::from_secs(30));
        options.set_clean_session(true);
        if let Some(user) = username {
            options.set_credentials(user, password.unwrap_or_default());
        }

        let (client, mut eventloop) = V311Client::new(options, 64);
        *MQTT_CLIENT.lock().unwrap() = Some(MqttClientEnum::V311(client));

        tauri::async_runtime::spawn(async move {
            loop {
                match eventloop.poll().await {
                    Ok(V311Event::Incoming(V311Packet::ConnAck(_))) => {
                        *MQTT_CONNECTED.lock().unwrap() = true;
                        app.emit("mqtt_connected", ()).ok();
                    }
                    Ok(V311Event::Incoming(V311Packet::Publish(p))) => {
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
                        let disconnecting = check_and_clear_disconnecting();
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
    }

    Ok(())
}

#[tauri::command]
pub async fn mqtt_disconnect() -> Result<(), String> {
    *MQTT_DISCONNECTING.lock().unwrap() = true;
    *MQTT_CONNECTED.lock().unwrap() = false;
    let client = MQTT_CLIENT.lock().unwrap().take();
    match client {
        Some(MqttClientEnum::V311(c)) => { c.disconnect().await.ok(); }
        Some(MqttClientEnum::V5(c)) => { c.disconnect().await.ok(); }
        None => {}
    }
    Ok(())
}

#[tauri::command]
pub fn mqtt_is_connected() -> bool {
    *MQTT_CONNECTED.lock().unwrap()
}

#[tauri::command]
pub async fn mqtt_subscribe(topic: String, qos: u8) -> Result<(), String> {
    let client = {
        let guard = MQTT_CLIENT.lock().unwrap();
        match guard.as_ref() {
            Some(MqttClientEnum::V311(c)) => Either::Left(c.clone()),
            Some(MqttClientEnum::V5(c)) => Either::Right(c.clone()),
            None => return Err("Not connected".into()),
        }
    };
    match client {
        Either::Left(c) => c.subscribe(topic, u8_to_qos(qos)).await.map_err(|e| e.to_string()),
        Either::Right(c) => c.subscribe(topic, u8_to_v5_qos(qos)).await.map_err(|e| e.to_string()),
    }
}

#[tauri::command]
pub async fn mqtt_unsubscribe(topic: String) -> Result<(), String> {
    let client = {
        let guard = MQTT_CLIENT.lock().unwrap();
        match guard.as_ref() {
            Some(MqttClientEnum::V311(c)) => Either::Left(c.clone()),
            Some(MqttClientEnum::V5(c)) => Either::Right(c.clone()),
            None => return Err("Not connected".into()),
        }
    };
    match client {
        Either::Left(c) => c.unsubscribe(topic).await.map_err(|e| e.to_string()),
        Either::Right(c) => c.unsubscribe(topic).await.map_err(|e| e.to_string()),
    }
}

#[tauri::command]
pub async fn mqtt_publish(
    topic: String,
    payload: String,
    qos: u8,
    retain: bool,
) -> Result<(), String> {
    let client = {
        let guard = MQTT_CLIENT.lock().unwrap();
        match guard.as_ref() {
            Some(MqttClientEnum::V311(c)) => Either::Left(c.clone()),
            Some(MqttClientEnum::V5(c)) => Either::Right(c.clone()),
            None => return Err("Not connected".into()),
        }
    };
    match client {
        Either::Left(c) => c
            .publish(topic, u8_to_qos(qos), retain, payload.into_bytes())
            .await
            .map_err(|e| e.to_string()),
        Either::Right(c) => c
            .publish(topic, u8_to_v5_qos(qos), retain, payload.into_bytes())
            .await
            .map_err(|e| e.to_string()),
    }
}

enum Either<L, R> {
    Left(L),
    Right(R),
}
