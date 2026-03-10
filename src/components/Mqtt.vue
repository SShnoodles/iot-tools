<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, nextTick } from "vue";
import { useI18n } from "vue-i18n";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { ElMessage } from "element-plus";
import { useStatusBar } from "../composables/useStatusBar";

const { t } = useI18n();
const { setSegments, clearSegments } = useStatusBar();

interface MqttMessage {
  topic: string;
  payload: string;
  qos: number;
  retain: boolean;
}

// Connection
const host = ref("localhost");
const port = ref(1883);
const clientId = ref(`iot-tools-${Math.random().toString(36).slice(2, 8)}`);
const username = ref("");
const password = ref("");
const protocol = ref("v311");
const isConnected = ref(false);
const connecting = ref(false);

const protocolOptions = [
  { label: "3.1.1", value: "v311" },
  { label: "5.0", value: "v5" },
];

// Subscribe
const subTopic = ref("");
const subQos = ref(0);
const subscriptions = ref<string[]>([]);

// Publish
const pubTopic = ref("");
const pubPayload = ref("");
const pubQos = ref(0);
const retain = ref(false);
const publishing = ref(false);

// Log
const logContent = ref("");
const logTextarea = ref<any>();

const qosOptions = [
  { label: "QoS 0", value: 0 },
  { label: "QoS 1", value: 1 },
  { label: "QoS 2", value: 2 },
];

function getTimestamp(): string {
  const now = new Date();
  const hh = String(now.getHours()).padStart(2, "0");
  const mm = String(now.getMinutes()).padStart(2, "0");
  const ss = String(now.getSeconds()).padStart(2, "0");
  const ms = String(now.getMilliseconds()).padStart(3, "0");
  return `${hh}:${mm}:${ss}.${ms}`;
}

function appendLog(line: string) {
  logContent.value += line + "\n";
  const lines = logContent.value.split("\n");
  if (lines.length > 500) logContent.value = lines.slice(-300).join("\n");
  nextTick(() => {
    const el =
      (logTextarea.value as any)?.$el?.querySelector("textarea") ||
      logTextarea.value;
    if (el) el.scrollTop = el.scrollHeight;
  });
}

async function connect() {
  if (!host.value) { ElMessage.error(t("mqtt.inputHost")); return; }
  connecting.value = true;
  try {
    await invoke("mqtt_connect", {
      host: host.value,
      port: port.value,
      clientId: clientId.value,
      username: username.value || null,
      password: password.value || null,
      protocol: protocol.value,
    });
  } catch (e) {
    ElMessage.error(t("mqtt.connectFailed") + e);
    connecting.value = false;
  }
}

async function disconnect() {
  try {
    await invoke("mqtt_disconnect");
    isConnected.value = false;
    subscriptions.value = [];
    appendLog(`# ${t("mqtt.disconnectedMsg")}`);
    ElMessage.info(t("mqtt.disconnectedMsg"));
  } catch (e) {
    ElMessage.error(String(e));
  }
}

async function subscribe() {
  if (!subTopic.value.trim()) { ElMessage.warning(t("mqtt.inputTopic")); return; }
  const topic = subTopic.value.trim();
  try {
    await invoke("mqtt_subscribe", { topic, qos: subQos.value });
    if (!subscriptions.value.includes(topic)) {
      subscriptions.value.push(topic);
    }
    appendLog(`# ${t("mqtt.subscribed")}: ${topic} (QoS ${subQos.value})`);
    subTopic.value = "";
  } catch (e) {
    ElMessage.error(t("mqtt.subscribeFailed") + e);
  }
}

async function unsubscribe(topic: string) {
  try {
    await invoke("mqtt_unsubscribe", { topic });
    subscriptions.value = subscriptions.value.filter((t) => t !== topic);
    appendLog(`# ${t("mqtt.unsubscribed")}: ${topic}`);
  } catch (e) {
    ElMessage.error(String(e));
  }
}

async function publish() {
  if (!pubTopic.value.trim()) { ElMessage.warning(t("mqtt.inputTopic")); return; }
  publishing.value = true;
  try {
    await invoke("mqtt_publish", {
      topic: pubTopic.value.trim(),
      payload: pubPayload.value,
      qos: pubQos.value,
      retain: retain.value,
    });
    const ts = getTimestamp();
    appendLog(`${ts} ↑ [${pubTopic.value}] ${pubPayload.value}`);
  } catch (e) {
    ElMessage.error(t("mqtt.publishFailed") + e);
  } finally {
    publishing.value = false;
  }
}

function clearLog() {
  logContent.value = "";
}

function updateStatusBar() {
  setSegments([
    {
      label: `${host.value}:${port.value}`,
      tag: {
        text: isConnected.value ? t("common.connected") : t("common.notConnected"),
        type: isConnected.value ? "success" : "danger",
      },
    },
  ]);
}

watch([host, port, isConnected], updateStatusBar);

const unlisteners: UnlistenFn[] = [];

onMounted(async () => {
  updateStatusBar();
  unlisteners.push(
    await listen("mqtt_connected", () => {
      isConnected.value = true;
      connecting.value = false;
      ElMessage.success(t("mqtt.connectSuccess"));
      appendLog(`# ${t("mqtt.connectedTo", { host: host.value, port: port.value })}`);
    })
  );
  unlisteners.push(
    await listen<string>("mqtt_disconnected", (event) => {
      isConnected.value = false;
      connecting.value = false;
      subscriptions.value = [];
      const reason = event.payload;
      appendLog(`# ${t("mqtt.connLost")}${reason ? ": " + reason : ""}`);
      if (reason) ElMessage.warning(reason);
    })
  );
  unlisteners.push(
    await listen<MqttMessage>("mqtt_message", (event) => {
      const { topic, payload, qos, retain } = event.payload;
      const ts = getTimestamp();
      const flag = retain ? " [R]" : "";
      appendLog(`${ts} ↓ [${topic}] (QoS ${qos}${flag}) ${payload}`);
    })
  );
});

onUnmounted(async () => {
  unlisteners.forEach((fn) => fn());
  if (isConnected.value) await invoke("mqtt_disconnect");
  clearSegments();
});
</script>

<template>
  <!-- Connection -->
  <el-form label-position="right" label-width="80px" :inline="true" size="small" @submit.prevent>
    <el-form-item :label="t('mqtt.host')">
      <el-input v-model="host" :disabled="isConnected || connecting" placeholder="localhost"
        autocomplete="off" autocorrect="off" autocapitalize="off" spellcheck="false" />
    </el-form-item>
    <el-form-item :label="t('mqtt.port')">
      <el-input-number v-model="port" :min="1" :max="65535" :precision="0"
        controls-position="right" :disabled="isConnected || connecting" />
    </el-form-item>
    <el-form-item :label="t('mqtt.clientId')">
      <el-input v-model="clientId" :disabled="isConnected || connecting"
        autocomplete="off" autocorrect="off" autocapitalize="off" spellcheck="false" />
    </el-form-item>
    <el-form-item :label="t('mqtt.username')">
      <el-input v-model="username" :disabled="isConnected || connecting" :placeholder="t('mqtt.optional')"
        autocomplete="off" autocorrect="off" autocapitalize="off" spellcheck="false" />
    </el-form-item>
    <el-form-item :label="t('mqtt.password')">
      <el-input v-model="password" :disabled="isConnected || connecting" show-password :placeholder="t('mqtt.optional')"
        autocomplete="new-password" autocorrect="off" autocapitalize="off" spellcheck="false" />
    </el-form-item>
    <el-form-item :label="t('mqtt.protocol')">
      <el-select v-model="protocol" :disabled="isConnected || connecting" style="width: 100px">
        <el-option v-for="o in protocolOptions" :key="o.value" :label="o.label" :value="o.value" />
      </el-select>
    </el-form-item>
    <el-form-item>
      <el-button type="primary" @click="connect" v-if="!isConnected" :loading="connecting">
        {{ t('mqtt.connect') }}
      </el-button>
      <el-button type="danger" @click="disconnect" v-else>
        {{ t('mqtt.disconnect') }}
      </el-button>
    </el-form-item>
  </el-form>

  <!-- Subscribe & Publish -->
  <el-row :gutter="12" style="margin-top: 6px;">
    <!-- Subscribe -->
    <el-col :span="12">
      <div class="panel">
        <el-text class="panel-title" size="small">{{ t('mqtt.subscribe') }}</el-text>
        <el-form size="small" @submit.prevent style="margin-top: 6px;">
          <el-form-item style="margin-bottom: 6px;">
            <el-input v-model="subTopic" :placeholder="t('mqtt.inputTopic')"
              :disabled="!isConnected" style="width: 100%"
              autocomplete="off" autocorrect="off" autocapitalize="off" spellcheck="false" />
          </el-form-item>
          <el-form-item style="margin-bottom: 0;">
            <el-select v-model="subQos" style="width: 110px" :disabled="!isConnected">
              <el-option v-for="o in qosOptions" :key="o.value" :label="o.label" :value="o.value" />
            </el-select>
            <el-button type="primary" @click="subscribe" :disabled="!isConnected" style="margin-left: 6px;">
              {{ t('mqtt.subscribe') }}
            </el-button>
          </el-form-item>
        </el-form>
        <div style="margin-top: 6px; min-height: 28px;">
          <el-tag
            v-for="topic in subscriptions" :key="topic"
            closable @close="unsubscribe(topic)"
            size="small" style="margin: 2px 4px 2px 0;"
          >{{ topic }}</el-tag>
          <el-text v-if="!subscriptions.length" size="small" type="info">{{ t('mqtt.noSubscriptions') }}</el-text>
        </div>
      </div>
    </el-col>

    <!-- Publish -->
    <el-col :span="12">
      <div class="panel">
        <el-text class="panel-title" size="small">{{ t('mqtt.publish') }}</el-text>
        <el-form size="small" @submit.prevent style="margin-top: 6px;">
          <el-form-item style="margin-bottom: 6px;">
            <el-input v-model="pubTopic" :placeholder="t('mqtt.inputTopic')"
              :disabled="!isConnected" style="width: 100%"
              autocomplete="off" autocorrect="off" autocapitalize="off" spellcheck="false" />
          </el-form-item>
          <el-form-item style="margin-bottom: 6px;">
            <el-input v-model="pubPayload" type="textarea" :rows="3"
              :placeholder="t('mqtt.inputPayload')" :disabled="!isConnected" />
          </el-form-item>
          <el-form-item style="margin-bottom: 0;">
            <el-select v-model="pubQos" style="width: 110px" :disabled="!isConnected">
              <el-option v-for="o in qosOptions" :key="o.value" :label="o.label" :value="o.value" />
            </el-select>
            <el-checkbox v-model="retain" :disabled="!isConnected" style="margin-left: 10px;">
              {{ t('mqtt.retain') }}
            </el-checkbox>
            <el-button type="primary" @click="publish" :disabled="!isConnected"
              :loading="publishing" style="margin-left: 10px;">
              {{ t('mqtt.publish') }}
            </el-button>
          </el-form-item>
        </el-form>
      </div>
    </el-col>
  </el-row>

  <!-- Message log -->
  <div style="margin-top: 8px; display: flex; align-items: center; justify-content: space-between; margin-bottom: 4px;">
    <el-text size="small" type="info">{{ t('mqtt.commLog') }}</el-text>
    <el-button size="small" @click="clearLog">{{ t('common.clear') }}</el-button>
  </div>
  <el-input type="textarea" ref="logTextarea" v-model="logContent"
    :rows="20" readonly style="font-family: monospace; font-size: 12px;" />
</template>

<style scoped>
.panel {
  border: 1px solid var(--el-border-color);
  border-radius: 4px;
  padding: 8px 10px;
}
.panel-title {
  color: var(--el-text-color-secondary);
  font-weight: 500;
}
</style>
