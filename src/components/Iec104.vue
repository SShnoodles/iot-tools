<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { ElMessage, ElMessageBox } from "element-plus";
import { useStatusBar } from "../composables/useStatusBar";

interface Iec104Point {
  ioa: number;
  value: string;
  quality: string;
  timestamp: string;
}

interface Iec104Frame {
  direction: "tx" | "rx";
  hex: string;
  format: string;
  type_id: number | null;
  type_name: string;
  send_sequence: number | null;
  receive_sequence: number | null;
  cause: number | null;
  common_address: number | null;
  summary: string;
  points: Iec104Point[];
}

interface PointRow extends Iec104Point {
  key: string;
  localTime: string;
  typeName: string;
  commonAddress: number;
  cause: number | null;
}

const { t, locale } = useI18n();
const { setSegments, clearSegments } = useStatusBar();

const host = ref("127.0.0.1");
const port = ref(2404);
const commonAddress = ref(1);
const timeoutMs = ref(5000);
const autoStart = ref(true);
const isConnected = ref(false);
const dataTransferStarted = ref(false);
const connecting = ref(false);
const sending = ref(false);
const rawHex = ref("68 04 43 00 00 00");
const logContent = ref("");
const logTextarea = ref<any>();
const points = ref<PointRow[]>([]);
const activeView = ref("objects");
const unlisteners: UnlistenFn[] = [];

const connectionLabel = computed(() => `${host.value}:${port.value}`);

function timestamp(): string {
  const now = new Date();
  return `${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}:${String(now.getSeconds()).padStart(2, "0")}.${String(now.getMilliseconds()).padStart(3, "0")}`;
}

function appendLog(line: string) {
  logContent.value += line + "\n";
  const lines = logContent.value.split("\n");
  if (lines.length > 1000) logContent.value = lines.slice(-600).join("\n");
  nextTick(() => {
    const element = logTextarea.value?.$el?.querySelector("textarea") || logTextarea.value;
    if (element) element.scrollTop = element.scrollHeight;
  });
}

function recordFrame(frame: Iec104Frame) {
  const time = timestamp();
  const arrow = frame.direction === "tx" ? "TX ->" : "RX <-";
  appendLog(`${time} ${arrow} ${frame.hex}`);
  appendLog(`             ${frame.summary}`);

  if (frame.direction === "rx" && frame.common_address != null) {
    for (const point of frame.points) {
      const key = `${frame.common_address}:${point.ioa}:${frame.type_id ?? 0}`;
      const row: PointRow = {
        ...point,
        key,
        localTime: time,
        typeName: frame.type_name,
        commonAddress: frame.common_address,
        cause: frame.cause,
      };
      const index = points.value.findIndex((item) => item.key === key);
      if (index >= 0) points.value[index] = row;
      else points.value.unshift(row);
    }
    if (points.value.length > 1000) points.value = points.value.slice(0, 1000);
  }

  if (frame.type_name === "STARTDT_CON") dataTransferStarted.value = true;
  if (frame.type_name === "STOPDT_CON") dataTransferStarted.value = false;
}

async function sendControl(control: "start" | "stop" | "test") {
  if (!isConnected.value) return;
  sending.value = true;
  try {
    const frame = await invoke<Iec104Frame>("iec104_send_control", { control });
    recordFrame(frame);
    if (control === "stop") dataTransferStarted.value = false;
  } catch (error) {
    ElMessage.error(t("iec104.sendFailed") + error);
    await refreshConnectionState();
  } finally {
    sending.value = false;
  }
}

async function connect() {
  if (!host.value.trim()) {
    ElMessage.warning(t("iec104.inputHost"));
    return;
  }
  connecting.value = true;
  try {
    await invoke("iec104_connect", {
      host: host.value.trim(),
      port: port.value,
      timeoutMs: timeoutMs.value,
    });
    isConnected.value = true;
    dataTransferStarted.value = false;
    appendLog(`# ${t("iec104.connectedTo", { host: host.value, port: port.value })}`);
    ElMessage.success(t("iec104.connectSuccess"));
    if (autoStart.value) await sendControl("start");
  } catch (error) {
    ElMessage.error(t("iec104.connectFailed") + error);
  } finally {
    connecting.value = false;
  }
}

async function disconnect() {
  try {
    await invoke("iec104_disconnect");
    isConnected.value = false;
    dataTransferStarted.value = false;
    appendLog(`# ${t("iec104.disconnectedMsg")}`);
    ElMessage.info(t("iec104.disconnectedMsg"));
  } catch (error) {
    ElMessage.error(String(error));
  }
}

async function generalInterrogation() {
  sending.value = true;
  try {
    const frame = await invoke<Iec104Frame>("iec104_general_interrogation", {
      commonAddress: commonAddress.value,
    });
    recordFrame(frame);
  } catch (error) {
    ElMessage.error(t("iec104.sendFailed") + error);
    await refreshConnectionState();
  } finally {
    sending.value = false;
  }
}

async function clockSync() {
  const previewTime = new Date().toLocaleString(
    locale.value === "zh" ? "zh-CN" : "en-US",
    { hour12: false },
  );
  try {
    await ElMessageBox.confirm(
      t("iec104.clockSyncConfirmMessage", {
        time: previewTime,
        address: commonAddress.value,
      }),
      t("iec104.clockSyncConfirmTitle"),
      {
        confirmButtonText: t("common.confirm"),
        cancelButtonText: t("common.cancel"),
        type: "warning",
      },
    );
  } catch {
    return;
  }

  const timestampMs = Date.now();
  sending.value = true;
  try {
    const frame = await invoke<Iec104Frame>("iec104_clock_sync", {
      commonAddress: commonAddress.value,
      timestampMs,
    });
    recordFrame(frame);
  } catch (error) {
    ElMessage.error(t("iec104.sendFailed") + error);
    await refreshConnectionState();
  } finally {
    sending.value = false;
  }
}

async function sendRaw() {
  if (!rawHex.value.trim()) {
    ElMessage.warning(t("iec104.inputRaw"));
    return;
  }
  sending.value = true;
  try {
    const frame = await invoke<Iec104Frame>("iec104_send_raw", { hex: rawHex.value });
    recordFrame(frame);
  } catch (error) {
    ElMessage.error(t("iec104.sendFailed") + error);
    await refreshConnectionState();
  } finally {
    sending.value = false;
  }
}

async function refreshConnectionState() {
  isConnected.value = await invoke<boolean>("iec104_is_connected");
  if (!isConnected.value) dataTransferStarted.value = false;
}

function clearLog() {
  logContent.value = "";
}

function clearPoints() {
  points.value = [];
}

function updateStatusBar() {
  setSegments([{
    label: connectionLabel.value,
    tag: {
      text: isConnected.value ? t("common.connected") : t("common.notConnected"),
      type: isConnected.value ? "success" : "danger",
    },
    extraTags: dataTransferStarted.value
      ? [{ text: t("iec104.started"), type: "success" as const }]
      : [],
  }]);
}

watch([host, port, isConnected, dataTransferStarted, locale], updateStatusBar);

onMounted(async () => {
  updateStatusBar();
  unlisteners.push(await listen<Iec104Frame>("iec104_frame", (event) => {
    recordFrame(event.payload);
  }));
  unlisteners.push(await listen<string>("iec104_disconnected", (event) => {
    isConnected.value = false;
    dataTransferStarted.value = false;
    appendLog(`# ${t("iec104.connLost")}: ${event.payload}`);
    ElMessage.warning(t("iec104.connLost"));
  }));
});

onUnmounted(async () => {
  unlisteners.forEach((unlisten) => unlisten());
  if (isConnected.value) await invoke("iec104_disconnect");
  clearSegments();
});
</script>

<template>
  <el-form label-position="right" label-width="92px" :inline="true" size="small" @submit.prevent>
    <el-form-item :label="t('iec104.host')">
      <el-input v-model="host" :disabled="isConnected || connecting" placeholder="127.0.0.1"
        style="width: 150px" autocomplete="off" autocorrect="off" autocapitalize="off" spellcheck="false" />
    </el-form-item>
    <el-form-item :label="t('iec104.port')">
      <el-input-number v-model="port" :min="1" :max="65535" :precision="0" controls-position="right"
        style="width: 100px" :disabled="isConnected || connecting" />
    </el-form-item>
    <el-form-item :label="t('iec104.commonAddress')">
      <el-input-number v-model="commonAddress" :min="0" :max="65535" :precision="0"
        controls-position="right" style="width: 100px" />
    </el-form-item>
    <el-form-item :label="t('iec104.timeout')">
      <el-input-number v-model="timeoutMs" :min="100" :max="60000" :step="500" :precision="0"
        controls-position="right" style="width: 110px" :disabled="isConnected || connecting" />
      <el-text size="small" type="info" style="margin-left: 4px">ms</el-text>
    </el-form-item>
    <el-form-item>
      <el-checkbox v-model="autoStart" :disabled="isConnected || connecting">{{ t('iec104.autoStart') }}</el-checkbox>
    </el-form-item>
    <el-form-item>
      <el-button v-if="!isConnected" type="primary" :loading="connecting" @click="connect">
        {{ t('iec104.connect') }}
      </el-button>
      <el-button v-else type="danger" @click="disconnect">{{ t('iec104.disconnect') }}</el-button>
    </el-form-item>
  </el-form>

  <div class="toolbar">
    <el-button size="small" type="success" :disabled="!isConnected || sending || dataTransferStarted"
      @click="sendControl('start')">{{ t('iec104.startDataTransfer') }}</el-button>
    <el-button size="small" type="warning" :disabled="!isConnected || sending || !dataTransferStarted"
      @click="sendControl('stop')">{{ t('iec104.stopDataTransfer') }}</el-button>
    <el-button size="small" :disabled="!isConnected || sending" @click="sendControl('test')">
      {{ t('iec104.testFrame') }}
    </el-button>
    <el-divider direction="vertical" />
    <el-button size="small" type="primary" :disabled="!isConnected || sending || !dataTransferStarted"
      @click="generalInterrogation">{{ t('iec104.generalInterrogation') }}</el-button>
    <el-button size="small" :disabled="!isConnected || sending || !dataTransferStarted"
      @click="clockSync">{{ t('iec104.clockSync') }}</el-button>
    <el-tag v-if="dataTransferStarted" size="small" type="success">{{ t('iec104.started') }}</el-tag>
  </div>

  <div class="raw-panel">
    <el-text size="small" type="info" class="raw-label">{{ t('iec104.rawApdu') }}</el-text>
    <el-input v-model="rawHex" :disabled="!isConnected" class="raw-input" size="small"
      placeholder="68 04 43 00 00 00" autocomplete="off" spellcheck="false" @keyup.enter="sendRaw" />
    <el-button size="small" type="primary" :loading="sending" :disabled="!isConnected" @click="sendRaw">
      {{ t('common.send') }}
    </el-button>
  </div>

  <el-tabs v-model="activeView" class="result-tabs">
    <el-tab-pane name="objects">
      <template #label>
        <span>{{ `${t('iec104.objects')} (${points.length})` }}</span>
        <el-button v-if="activeView === 'objects'" link type="primary" size="small" class="tab-clear"
          @click.stop="clearPoints">{{ t('common.clear') }}</el-button>
      </template>
      <el-table :data="points" size="small" border max-height="430" empty-text="-">
        <el-table-column prop="localTime" :label="t('iec104.localTime')" width="105" />
        <el-table-column prop="typeName" :label="t('iec104.type')" width="115" />
        <el-table-column prop="cause" label="COT" width="60" />
        <el-table-column prop="commonAddress" label="CA" width="70" />
        <el-table-column prop="ioa" label="IOA" width="90" />
        <el-table-column prop="value" :label="t('iec104.value')" min-width="150" />
        <el-table-column :label="t('iec104.quality')" width="95">
          <template #default="{ row }">
            <el-tag v-if="row.quality" size="small" :type="row.quality === 'Good' ? 'success' : 'warning'">
              {{ row.quality }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="timestamp" :label="t('iec104.sourceTime')" min-width="175" />
      </el-table>
    </el-tab-pane>
    <el-tab-pane name="log">
      <template #label>
        <span>{{ t('iec104.commLog') }}</span>
        <el-button v-if="activeView === 'log'" link type="primary" size="small" class="tab-clear"
          @click.stop="clearLog">{{ t('common.clear') }}</el-button>
      </template>
      <el-input ref="logTextarea" v-model="logContent" type="textarea" :rows="20" readonly
        class="log-input" />
    </el-tab-pane>
  </el-tabs>
</template>

<style scoped>
.toolbar {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 10px;
  border: 1px solid var(--el-border-color);
  border-radius: 4px;
}

.toolbar .el-button + .el-button {
  margin-left: 0;
}

.raw-panel {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 8px;
}

.raw-label {
  flex: 0 0 auto;
}

.raw-input {
  flex: 1;
  font-family: monospace;
}

.result-tabs {
  margin-top: 6px;
}

.tab-clear {
  margin-left: 8px;
  padding: 0;
  vertical-align: baseline;
}

.log-input :deep(textarea) {
  font-family: monospace;
  font-size: 12px;
}
</style>
