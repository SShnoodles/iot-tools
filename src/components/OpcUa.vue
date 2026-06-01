<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted } from "vue";
import { useI18n } from "vue-i18n";
import { invoke } from "@tauri-apps/api/core";
import { ElMessage } from "element-plus";
import { useStatusBar } from "../composables/useStatusBar";

const { t } = useI18n();
const { setSegments, clearSegments } = useStatusBar();

// Connection
const endpointUrl = ref("opc.tcp://localhost:4840/");
const username = ref("");
const password = ref("");
const isConnected = ref(false);
const connecting = ref(false);

// Node table
interface NodeValue {
  value: string;
  data_type: string;
  status: string;
  source_timestamp: string;
}

interface NodeRow {
  nodeId: string;
  value: string;
  dataType: string;
  status: string;
  sourceTimestamp: string;
  localTime: string;
  reading: boolean;
  editing: boolean;
  editValue: string;
  writing: boolean;
}
const rows = ref<NodeRow[]>([]);

// Add node
const addNodeId = ref("");
const adding = ref(false);
const timeoutMs = ref(5000);

function getTimestamp(): string {
  const now = new Date();
  const hh = String(now.getHours()).padStart(2, "0");
  const mm = String(now.getMinutes()).padStart(2, "0");
  const ss = String(now.getSeconds()).padStart(2, "0");
  const ms = String(now.getMilliseconds()).padStart(3, "0");
  return `${hh}:${mm}:${ss}.${ms}`;
}

async function connect() {
  if (!endpointUrl.value.trim()) {
    ElMessage.error(t("opcua.inputEndpoint"));
    return;
  }
  connecting.value = true;
  try {
    await invoke("opcua_connect", {
      endpointUrl: endpointUrl.value.trim(),
      username: username.value || null,
      password: password.value || null,
    });
    isConnected.value = true;
    ElMessage.success(t("opcua.connectSuccess"));
  } catch (e) {
    ElMessage.error(t("opcua.connectFailed") + e);
  } finally {
    connecting.value = false;
  }
}

async function disconnect() {
  try {
    await invoke("opcua_disconnect");
    isConnected.value = false;
    rows.value = [];
    ElMessage.info(t("opcua.disconnectedMsg"));
  } catch (e) {
    ElMessage.error(String(e));
  }
}

async function addAndRead() {
  const nid = addNodeId.value.trim();
  if (!nid) { ElMessage.warning(t("opcua.inputNodeId")); return; }

  // Avoid duplicates
  if (rows.value.find(r => r.nodeId === nid)) {
    await readRow(rows.value.find(r => r.nodeId === nid)!);
    addNodeId.value = "";
    return;
  }

  adding.value = true;
  const row: NodeRow = { nodeId: nid, value: "", dataType: "", status: "", sourceTimestamp: "", localTime: "", reading: true, editing: false, editValue: "", writing: false };
  rows.value.push(row);

  try {
    const nv: NodeValue = await invoke("opcua_read_node", { nodeId: nid, timeoutMs: timeoutMs.value });
    row.value = nv.value;
    row.dataType = nv.data_type;
    row.status = nv.status;
    row.sourceTimestamp = nv.source_timestamp;
    row.localTime = getTimestamp();
    addNodeId.value = "";
  } catch (e) {
    row.value = `Error: ${e}`;
    row.status = "Error";
    row.localTime = getTimestamp();
    ElMessage.error(t("opcua.readFailed") + e);
  } finally {
    row.reading = false;
    adding.value = false;
  }
}

async function readRow(row: NodeRow) {
  row.reading = true;
  try {
    const nv: NodeValue = await invoke("opcua_read_node", { nodeId: row.nodeId, timeoutMs: timeoutMs.value });
    row.value = nv.value;
    row.dataType = nv.data_type;
    row.status = nv.status;
    row.sourceTimestamp = nv.source_timestamp;
    row.localTime = getTimestamp();
  } catch (e) {
    row.value = `Error: ${e}`;
    row.status = "Error";
    row.localTime = getTimestamp();
    ElMessage.error(t("opcua.readFailed") + e);
  } finally {
    row.reading = false;
  }
}

function startEdit(row: NodeRow) {
  row.editValue = row.value.startsWith("Error:") ? "" : row.value;
  row.editing = true;
}

function cancelEdit(row: NodeRow) {
  row.editing = false;
}

async function writeRow(row: NodeRow) {
  row.writing = true;
  try {
    await invoke("opcua_write_node", { nodeId: row.nodeId, value: row.editValue });
    row.value = row.editValue;
    row.localTime = getTimestamp();
    row.editing = false;
    ElMessage.success(t("opcua.writeSuccess"));
  } catch (e) {
    ElMessage.error(t("opcua.writeFailed") + e);
  } finally {
    row.writing = false;
  }
}

function removeRow(row: NodeRow) {
  rows.value = rows.value.filter(r => r !== row);
}

function updateStatusBar() {
  setSegments([
    {
      label: endpointUrl.value,
      tag: {
        text: isConnected.value ? t("common.connected") : t("common.notConnected"),
        type: isConnected.value ? "success" : "danger",
      },
    },
  ]);
}

watch([endpointUrl, isConnected], updateStatusBar);
onMounted(() => { updateStatusBar(); });
onUnmounted(async () => {
  if (isConnected.value) await invoke("opcua_disconnect");
  clearSegments();
});
</script>

<template>
  <!-- Connection -->
  <el-form label-position="right" label-width="60px" :inline="true" size="small" @submit.prevent>
    <el-form-item :label="t('opcua.endpoint')">
      <el-input
        v-model="endpointUrl"
        :disabled="isConnected || connecting"
        placeholder="opc.tcp://localhost:4840/"
        style="width: 200px"
        autocomplete="off" autocorrect="off" autocapitalize="off" spellcheck="false"
      />
    </el-form-item>
    <el-form-item :label="t('opcua.username')">
      <el-input
        v-model="username"
        :disabled="isConnected || connecting"
        :placeholder="t('opcua.optional')"
        autocomplete="off" autocorrect="off" autocapitalize="off" spellcheck="false"
      />
    </el-form-item>
    <el-form-item :label="t('opcua.password')">
      <el-input
        v-model="password"
        :disabled="isConnected || connecting"
        show-password
        :placeholder="t('opcua.optional')"
        autocomplete="new-password" autocorrect="off" autocapitalize="off" spellcheck="false"
      />
    </el-form-item>
    <el-form-item>
      <el-button type="primary" @click="connect" v-if="!isConnected" :loading="connecting">
        {{ t('opcua.connect') }}
      </el-button>
      <el-button type="danger" @click="disconnect" v-else>
        {{ t('opcua.disconnect') }}
      </el-button>
    </el-form-item>
  </el-form>

  <!-- Add node -->
  <el-form :inline="true" size="small" label-width="60px" @submit.prevent style="margin-top: 4px;">
    <el-form-item :label="t('opcua.nodeId')">
      <el-input
        v-model="addNodeId"
        :placeholder="t('opcua.nodeIdPlaceholder')"
        :disabled="!isConnected"
        style="width: 200px"
        autocomplete="off" autocorrect="off" autocapitalize="off" spellcheck="false"
        @keyup.enter="addAndRead"
      />
    </el-form-item>
    <el-form-item :label="t('opcua.timeout')">
      <el-input-number
        v-model="timeoutMs"
        :min="100"
        :max="60000"
        :step="500"
        :precision="0"
        controls-position="right"
        style="width: 120px"
      />
      <el-text size="small" type="info" style="margin-left: 4px;">ms</el-text>
    </el-form-item>
    <el-form-item>
      <el-button type="primary" :disabled="!isConnected" :loading="adding" @click="addAndRead">
        {{ t('opcua.addRead') }}
      </el-button>
    </el-form-item>
  </el-form>

  <!-- Node table -->
  <el-table :data="rows" size="small" border style="margin-top: 6px; width: 100%;">
    <el-table-column :label="t('opcua.nodeId')" prop="nodeId" min-width="160" />
    <el-table-column :label="t('opcua.value')" min-width="180">
      <template #default="{ row }">
        <template v-if="row.editing">
          <el-input
            v-model="row.editValue"
            size="small"
            style="width: 120px"
            @keyup.enter="writeRow(row)"
            @keyup.escape="cancelEdit(row)"
          />
          <el-button size="small" type="primary" :loading="row.writing" style="margin-left: 4px;" @click="writeRow(row)">
            {{ t('opcua.write') }}
          </el-button>
          <el-button size="small" style="margin-left: 4px;" @click="cancelEdit(row)">
            {{ t('common.cancel') }}
          </el-button>
        </template>
        <template v-else>
          <span class="value-cell" :class="{ 'value-error': row.status === 'Error' }">{{ row.value }}</span>
          <el-button
            v-if="isConnected && row.value && row.status !== 'Error'"
            size="small" link type="primary" style="margin-left: 6px;"
            @click="startEdit(row)"
          >{{ t('opcua.edit') }}</el-button>
        </template>
      </template>
    </el-table-column>
    <el-table-column :label="t('opcua.dataType')" prop="dataType" width="90" />
    <el-table-column :label="t('opcua.status')" width="90">
      <template #default="{ row }">
        <el-tag
          v-if="row.status"
          size="small"
          :type="row.status === 'Good' ? 'success' : row.status.startsWith('Uncertain') ? 'warning' : 'danger'"
        >{{ row.status }}</el-tag>
      </template>
    </el-table-column>
    <el-table-column :label="t('opcua.sourceTimestamp')" prop="sourceTimestamp" min-width="150" />
    <el-table-column :label="t('opcua.localTime')" prop="localTime" width="100" />
    <el-table-column :label="t('opcua.actions')" width="100" align="center">
      <template #default="{ row }">
        <el-button size="small" link type="primary" :loading="row.reading" :disabled="!isConnected" @click="readRow(row)">
          {{ t('opcua.refresh') }}
        </el-button>
        <el-button size="small" link type="danger" style="margin-left: 6px;" @click="removeRow(row)">
          {{ t('common.delete') }}
        </el-button>
      </template>
    </el-table-column>
    <template #empty>
      <el-text type="info" size="small">{{ t('opcua.noNodes') }}</el-text>
    </template>
  </el-table>
</template>

<style scoped>
.value-cell {
  font-family: monospace;
  font-size: 12px;
}
.value-error {
  color: var(--el-color-danger);
}
</style>
