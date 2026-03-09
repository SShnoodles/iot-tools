<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, nextTick, watch } from "vue";
import { useI18n } from "vue-i18n";
import { invoke } from "@tauri-apps/api/core";
import { ElMessage } from "element-plus";
import { useStatusBar } from '../composables/useStatusBar';

const { t } = useI18n();
const { setSegments, clearSegments } = useStatusBar();

function updateStatusBar() {
  const extraTags = isPolling.value ? [{ text: t('modbus.polling'), type: 'warning' as const }] : [];
  setSegments([{
    label: `${host.value}:${port.value}`,
    tag: {
      text: isConnected.value ? t('common.connected') : t('common.notConnected'),
      type: isConnected.value ? 'success' : 'danger',
    },
    extraTags,
  }]);
}

interface ModbusResponse {
  request_hex: string;
  response_hex: string;
  values: number[];
  exception_code: number | null;
  display_value: string;
}

interface ParsedValue {
  address: number;
  dec: number;
  hex: string;
  binary: string;
  display_value: string;
}

const host = ref("192.168.1.1");
const port = ref(502);
const unitId = ref(1);
const functionCode = ref(3);
const startAddress = ref(0);
const quantity = ref(10);
const writeValue = ref(0);

const isConnected = ref(false);
const connecting = ref(false);
const sending = ref(false);
const logContent = ref("");
const parsedValues = ref<ParsedValue[]>([]);

const logTextarea = ref<any>();
const logCollapse = ref<string[]>([]);

const pollingTimer = ref<ReturnType<typeof setInterval> | null>(null);
const isPolling = ref(false);

const displayFormat = ref("Unsigned");

const displayFormatOptions = [
  { label: "Unsigned",      value: "Unsigned"      },
  { label: "Signed",        value: "Signed"        },
  { label: "Hex",           value: "Hex"           },
  { label: "Binary",        value: "Binary"        },
  { label: "Long",          value: "Long"          },
  { label: "Long Inverse",  value: "LongInverse"   },
  { label: "Float",         value: "Float"         },
  { label: "Float Inverse", value: "FloatInverse"  },
  { label: "Double",        value: "Double"        },
  { label: "Double Inverse",value: "DoubleInverse" },
];

const functionCodeOptions = computed(() => [
  { label: t('modbus.fc.readCoils'),            value: 1 },
  { label: t('modbus.fc.readDiscreteInputs'),   value: 2 },
  { label: t('modbus.fc.readHoldingRegisters'), value: 3 },
  { label: t('modbus.fc.readInputRegisters'),   value: 4 },
  { label: t('modbus.fc.writeSingleCoil'),      value: 5 },
  { label: t('modbus.fc.writeSingleRegister'),  value: 6 },
]);

const isReadFunction  = computed(() => [1, 2, 3, 4].includes(functionCode.value));
const isWriteFunction = computed(() => [5, 6].includes(functionCode.value));

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
    const el = (logTextarea.value as any)?.$el?.querySelector("textarea") || logTextarea.value;
    if (el) el.scrollTop = el.scrollHeight;
  });
}

function getExceptionMessage(code: number): string {
  const messages: Record<number, string> = {
    0x01: t('modbus.exception.illegalFunction'),
    0x02: t('modbus.exception.illegalAddress'),
    0x03: t('modbus.exception.illegalValue'),
    0x04: t('modbus.exception.deviceFailure'),
    0x05: t('modbus.exception.acknowledge'),
    0x06: t('modbus.exception.deviceBusy'),
    0x08: t('modbus.exception.memoryError'),
    0x0a: t('modbus.exception.gatewayPath'),
    0x0b: t('modbus.exception.gatewayTarget'),
  };
  return messages[code] ?? t('modbus.exception.unknown');
}

async function connect() {
  if (!host.value) {
    ElMessage.error(t('modbus.inputHost'));
    return;
  }
  connecting.value = true;
  try {
    await invoke("modbus_tcp_connect", { host: host.value, port: port.value });
    isConnected.value = true;
    ElMessage.success(t('modbus.connectSuccess'));
    appendLog(`# ${t('modbus.connectedTo', { host: host.value, port: port.value })}`);
  } catch (e) {
    ElMessage.error(t('modbus.connectFailed') + e);
  } finally {
    connecting.value = false;
  }
}

async function disconnect() {
  stopPolling();
  await invoke("modbus_tcp_disconnect");
  isConnected.value = false;
  ElMessage.info(t('modbus.disconnectedMsg'));
  appendLog(`# ${t('modbus.disconnectedMsg')}`);
}

async function send() {
  if (!isConnected.value) {
    ElMessage.error(t('modbus.connectFirst'));
    return;
  }
  sending.value = true;
  parsedValues.value = [];

  const qty  = isReadFunction.value  ? quantity.value    : 1;
  const vals = isWriteFunction.value ? [writeValue.value] : [];

  try {
    const result = await invoke<ModbusResponse>("modbus_tcp_send", {
      unitId: unitId.value,
      functionCode: functionCode.value,
      address: startAddress.value,
      quantity: qty,
      values: vals,
      displayFormat: displayFormat.value,
    });

    const ts = getTimestamp();
    appendLog(`${ts} TX -> ${result.request_hex}`);
    appendLog(`${ts} RX <- ${result.response_hex}`);

    if (result.exception_code != null) {
      const code = result.exception_code;
      appendLog(`               Exception: ${getExceptionMessage(code)} (0x${code.toString(16).padStart(2, "0").toUpperCase()})`);
    } else if (isReadFunction.value && result.values.length > 0) {
      parsedValues.value = result.values.map((val, idx) => ({
        address: startAddress.value + idx,
        dec: val,
        hex: "0x" + val.toString(16).toUpperCase().padStart(4, "0"),
        binary: val.toString(2).padStart(16, "0"),
        display_value: idx === 0 ? result.display_value : "",
      }));
    }

    const connected = await invoke<boolean>("modbus_tcp_is_connected");
    if (!connected && isConnected.value) {
      isConnected.value = false;
      appendLog(`# ${t('modbus.connLost')}`);
    }
  } catch (e) {
    const ts = getTimestamp();
    appendLog(`${ts} ERROR: ${e}`);
    ElMessage.error(t('modbus.sendFailed') + e);
    isConnected.value = await invoke<boolean>("modbus_tcp_is_connected");
    if (!isConnected.value) appendLog(`# ${t('modbus.connLost')}`);
  } finally {
    sending.value = false;
  }
}

function clearLog() {
  logContent.value = "";
  parsedValues.value = [];
}

function startPolling() {
  if (pollingTimer.value) return;
  isPolling.value = true;
  send();
  pollingTimer.value = setInterval(() => { if (!sending.value) send(); }, 1000);
}

function stopPolling() {
  if (pollingTimer.value) {
    clearInterval(pollingTimer.value);
    pollingTimer.value = null;
  }
  isPolling.value = false;
}

function togglePolling() {
  if (isPolling.value) stopPolling();
  else startPolling();
}

watch(functionCode, () => { if (!isReadFunction.value) stopPolling(); });
watch([host, port, isConnected, isPolling], updateStatusBar);

onMounted(() => { updateStatusBar(); });

onUnmounted(async () => {
  stopPolling();
  if (isConnected.value) await invoke("modbus_tcp_disconnect");
  clearSegments();
});
</script>

<template>
  <el-form label-position="right" label-width="100px" :inline="true" size="small" @submit.prevent>
    <el-form-item :label="t('modbus.host')">
      <el-input v-model="host" style="width: 150px" :disabled="isConnected" placeholder="192.168.1.1" />
    </el-form-item>
    <el-form-item :label="t('modbus.port')">
      <el-input-number v-model="port" :min="1" :max="65535" :precision="0" controls-position="right" style="width: 100px" :disabled="isConnected" />
    </el-form-item>
    <el-form-item :label="t('modbus.unitId')">
      <el-input-number v-model="unitId" :min="0" :max="255" :precision="0" controls-position="right" style="width: 80px" :disabled="isConnected" />
    </el-form-item>
    <el-form-item>
      <el-button type="primary" @click="connect" v-if="!isConnected" :loading="connecting">{{ t('modbus.connect') }}</el-button>
      <el-button type="danger" @click="disconnect" v-else>{{ t('modbus.disconnect') }}</el-button>
    </el-form-item>
  </el-form>

  <el-form label-position="right" label-width="100px" :inline="true" size="small" @submit.prevent>
    <el-form-item :label="t('modbus.functionCode')">
      <el-select v-model="functionCode" style="width: 150px">
        <el-option v-for="item in functionCodeOptions" :key="item.value" :label="item.label" :value="item.value" />
      </el-select>
    </el-form-item>
    <el-form-item :label="t('modbus.startAddress')">
      <el-input-number v-model="startAddress" :min="0" :max="65535" :precision="0" controls-position="right" style="width: 100px" />
    </el-form-item>
    <el-form-item :label="t('modbus.quantity')" v-if="isReadFunction">
      <el-input-number v-model="quantity" :min="1" :max="125" :precision="0" controls-position="right" style="width: 80px" />
    </el-form-item>
    <el-form-item>
      <el-button type="primary" @click="send" :disabled="!isConnected || isPolling" :loading="sending && !isPolling">{{ t('common.send') }}</el-button>
      <el-button
        v-if="isReadFunction"
        :type="isPolling ? 'warning' : 'success'"
        @click="togglePolling"
        :disabled="!isConnected"
        style="margin-left: 8px;"
      >{{ isPolling ? t('modbus.stopPolling') : t('modbus.startPolling') }}</el-button>
    </el-form-item>
    <el-form-item :label="t('modbus.displayFormat')" v-if="isReadFunction">
      <el-select v-model="displayFormat" style="width: 130px">
        <el-option v-for="item in displayFormatOptions" :key="item.value" :label="item.label" :value="item.value" />
      </el-select>
    </el-form-item>
  </el-form>

  <el-form label-position="right" label-width="80px" size="small" v-if="isWriteFunction" @submit.prevent>
    <el-form-item :label="t('modbus.writeData')">
      <el-input-number v-model="writeValue" :min="0" :max="functionCode === 5 ? 1 : 65535" :precision="0" controls-position="right" style="width: 150px" />
      <el-text style="margin-left: 8px; color: #909399;">
        {{ functionCode === 5 ? "(0=OFF, 1=ON)" : "(0-65535)" }}
      </el-text>
    </el-form-item>
  </el-form>

  <el-table
    v-show="isReadFunction"
    :data="parsedValues"
    size="small"
    border
    style="width: 100%; margin-top: 4px;"
  >
    <el-table-column prop="address" :label="t('modbus.address')" width="100" />
    <el-table-column prop="hex" :label="t('modbus.hex')" />
    <el-table-column prop="binary" :label="t('modbus.binary16')" />
    <el-table-column prop="display_value" :label="displayFormatOptions.find(o => o.value === displayFormat)?.label ?? displayFormat" />
  </el-table>

  <el-collapse v-model="logCollapse" style="margin-top: 8px;">
    <el-collapse-item name="log">
      <template #title>
        <span>{{ t('modbus.commLog') }}</span>
        <el-button size="small" @click.stop="clearLog" style="margin-left: 8px;">{{ t('common.clear') }}</el-button>
      </template>
      <el-input
        type="textarea"
        ref="logTextarea"
        v-model="logContent"
        :rows="5"
        readonly
        style="font-family: monospace;"
      />
    </el-collapse-item>
  </el-collapse>
</template>
