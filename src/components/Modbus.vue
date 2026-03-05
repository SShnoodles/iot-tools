<script setup lang="ts">
import { ref, computed, onUnmounted, nextTick, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { ElMessage } from "element-plus";

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
const logCollapse = ref<string[]>([]); // 默认折叠（空数组）

const pollingTimer = ref<ReturnType<typeof setInterval> | null>(null);
const isPolling = ref(false);

const displayFormat = ref("Unsigned");

const displayFormatOptions = [
  { label: "Unsigned", value: "Unsigned" },
  { label: "Signed", value: "Signed" },
  { label: "Hex", value: "Hex" },
  { label: "Binary", value: "Binary" },
  { label: "Long", value: "Long" },
  { label: "Long Inverse", value: "LongInverse" },
  { label: "Float", value: "Float" },
  { label: "Float Inverse", value: "FloatInverse" },
  { label: "Double", value: "Double" },
  { label: "Double Inverse", value: "DoubleInverse" },
];

const functionCodeOptions = [
  { label: "FC01 - Read Coils", value: 1 },
  { label: "FC02 - Read Discrete Inputs", value: 2 },
  { label: "FC03 - Read Holding Registers", value: 3 },
  { label: "FC04 - Read Input Registers", value: 4 },
  { label: "FC05 - Write Single Coil", value: 5 },
  { label: "FC06 - Write Single Register", value: 6 },
];

const isReadFunction = computed(() => [1, 2, 3, 4].includes(functionCode.value));
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
  if (lines.length > 500) {
    logContent.value = lines.slice(-300).join("\n");
  }
  nextTick(() => {
    const el =
      (logTextarea.value as any)?.$el?.querySelector("textarea") ||
      logTextarea.value;
    if (el) el.scrollTop = el.scrollHeight;
  });
}

function getExceptionMessage(code: number): string {
  const messages: Record<number, string> = {
    0x01: "非法功能码",
    0x02: "非法数据地址",
    0x03: "非法数据值",
    0x04: "从站设备故障",
    0x05: "确认",
    0x06: "从属设备忙",
    0x08: "内存奇偶校验错误",
    0x0a: "网关路径不可用",
    0x0b: "网关目标设备无响应",
  };
  return messages[code] ?? "未知异常";
}

async function connect() {
  if (!host.value) {
    ElMessage.error("请输入主机地址");
    return;
  }
  connecting.value = true;
  try {
    await invoke("modbus_tcp_connect", { host: host.value, port: port.value });
    isConnected.value = true;
    ElMessage.success("连接成功");
    appendLog(`# 已连接到 ${host.value}:${port.value}`);
  } catch (e) {
    ElMessage.error("连接失败: " + e);
  } finally {
    connecting.value = false;
  }
}

async function disconnect() {
  stopPolling();
  await invoke("modbus_tcp_disconnect");
  isConnected.value = false;
  ElMessage.info("已断开连接");
  appendLog("# 已断开连接");
}

async function send() {
  if (!isConnected.value) {
    ElMessage.error("请先连接");
    return;
  }
  sending.value = true;
  parsedValues.value = [];

  const qty = isReadFunction.value ? quantity.value : 1;
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
      appendLog(
        `               异常: ${getExceptionMessage(code)} (0x${code.toString(16).padStart(2, "0").toUpperCase()})`
      );
    } else if (isReadFunction.value && result.values.length > 0) {
      parsedValues.value = result.values.map((val, idx) => ({
        address: startAddress.value + idx,
        dec: val,
        hex: "0x" + val.toString(16).toUpperCase().padStart(4, "0"),
        binary: val.toString(2).padStart(16, "0"),
        display_value: idx === 0 ? result.display_value : "",
      }));
    }

    // Sync connection status in case I/O failed silently
    const connected = await invoke<boolean>("modbus_tcp_is_connected");
    if (!connected && isConnected.value) {
      isConnected.value = false;
      appendLog("# 连接已断开");
    }
  } catch (e) {
    const ts = getTimestamp();
    appendLog(`${ts} ERROR: ${e}`);
    ElMessage.error("发送失败: " + e);
    isConnected.value = await invoke<boolean>("modbus_tcp_is_connected");
    if (!isConnected.value) {
      appendLog("# 连接已断开");
    }
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
  pollingTimer.value = setInterval(() => {
    if (!sending.value) send();
  }, 1000);
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

// 切换到写功能码时停止轮询
watch(functionCode, () => {
  if (!isReadFunction.value) stopPolling();
});

onUnmounted(async () => {
  stopPolling();
  if (isConnected.value) {
    await invoke("modbus_tcp_disconnect");
  }
});
</script>

<template>
  <!-- 连接设置 -->
  <el-form label-position="right" label-width="80px" :inline="true" size="small" @submit.prevent>
    <el-form-item label="主机:">
      <el-input
        v-model="host"
        style="width: 150px"
        :disabled="isConnected"
        placeholder="192.168.1.1"
      />
    </el-form-item>
    <el-form-item label="端口:">
      <el-input-number
        v-model="port"
        :min="1"
        :max="65535"
        :precision="0"
        controls-position="right"
        style="width: 100px"
        :disabled="isConnected"
      />
    </el-form-item>
    <el-form-item label="单元ID:">
      <el-input-number
        v-model="unitId"
        :min="0"
        :max="255"
        :precision="0"
        controls-position="right"
        style="width: 80px"
        :disabled="isConnected"
      />
    </el-form-item>
    <el-form-item>
      <el-button
        type="primary"
        @click="connect"
        v-if="!isConnected"
        :loading="connecting"
      >连接</el-button>
      <el-button type="danger" @click="disconnect" v-else>断开</el-button>
    </el-form-item>
  </el-form>

  <!-- 请求设置 -->
  <el-form label-position="right" label-width="80px" :inline="true" size="small" @submit.prevent>
    <el-form-item label="功能码:">
      <el-select v-model="functionCode" style="width: 150px">
        <el-option
          v-for="item in functionCodeOptions"
          :key="item.value"
          :label="item.label"
          :value="item.value"
        />
      </el-select>
    </el-form-item>
    <el-form-item label="起始地址:">
      <el-input-number
        v-model="startAddress"
        :min="0"
        :max="65535"
        :precision="0"
        controls-position="right"
        style="width: 100px"
      />
    </el-form-item>
    <el-form-item label="数量:" v-if="isReadFunction">
      <el-input-number
        v-model="quantity"
        :min="1"
        :max="125"
        :precision="0"
        controls-position="right"
        style="width: 80px"
      />
    </el-form-item>
    <el-form-item>
      <el-button
        type="primary"
        @click="send"
        :disabled="!isConnected || isPolling"
        :loading="sending && !isPolling"
      >发送</el-button>
      <el-button
        v-if="isReadFunction"
        :type="isPolling ? 'warning' : 'success'"
        @click="togglePolling"
        :disabled="!isConnected"
        style="margin-left: 8px;"
      >{{ isPolling ? "停止刷新" : "实时刷新" }}</el-button>
    </el-form-item>
    <el-form-item label="显示格式:" v-if="isReadFunction">
      <el-select v-model="displayFormat" style="width: 130px">
        <el-option
          v-for="item in displayFormatOptions"
          :key="item.value"
          :label="item.label"
          :value="item.value"
        />
      </el-select>
    </el-form-item>
  </el-form>

  <!-- 写入数据 (FC05/FC06) -->
  <el-form
    label-position="right"
    label-width="80px"
    size="small"
    v-if="isWriteFunction"
    @submit.prevent
  >
    <el-form-item label="写入数据:">
      <el-input-number
        v-model="writeValue"
        :min="0"
        :max="functionCode === 5 ? 1 : 65535"
        :precision="0"
        controls-position="right"
        style="width: 150px"
      />
      <el-text style="margin-left: 8px; color: #909399;">
        {{ functionCode === 5 ? "(0=OFF, 1=ON)" : "(0-65535)" }}
      </el-text>
    </el-form-item>
  </el-form>

  <!-- 解析结果表格 (仅读操作) -->
  <el-table
    v-show="isReadFunction"
    :data="parsedValues"
    size="small"
    border
    style="width: 100%; margin-top: 4px;"
  >
    <el-table-column prop="address" label="地址" width="50" />
    <el-table-column prop="hex" label="十六进制" />
    <el-table-column prop="binary" label="二进制(16位)" />
    <el-table-column prop="display_value" :label="displayFormatOptions.find(o => o.value === displayFormat)?.label ?? displayFormat" />
  </el-table>

  <!-- 状态栏 -->
  <el-row style="margin-top: 8px;">
    <el-col :span="24">
      <el-text>
        {{ host }}:{{ port }}
        <el-tag
          :type="isConnected ? 'success' : 'danger'"
          size="small"
          style="margin-left: 8px;"
        >
          {{ isConnected ? "已连接" : "未连接" }}
        </el-tag>
        <el-tag
          v-if="isPolling"
          type="warning"
          size="small"
          style="margin-left: 8px;"
        >实时刷新中</el-tag>
      </el-text>
    </el-col>
  </el-row>

  <!-- 通信日志（底部，默认折叠） -->
  <el-collapse v-model="logCollapse" style="margin-top: 8px;">
    <el-collapse-item name="log">
      <template #title>
        <span>通信日志</span>
        <el-button size="small" @click="clearLog">清空</el-button>
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
