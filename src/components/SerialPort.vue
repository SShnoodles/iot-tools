<script setup lang="ts">
import { ref, reactive, onMounted, onUnmounted, nextTick, watch } from "vue";
import { useI18n } from "vue-i18n";
import { invoke } from "@tauri-apps/api/core";
import { listen } from '@tauri-apps/api/event';
import SerialPortSetting from "./SerialPortSetting.vue";
import { SerialPort, Option, SerialPortLog } from "../types/serial";
import { ElMessage } from 'element-plus';
import { useStatusBar } from '../composables/useStatusBar';

const { t } = useI18n();
const { setSegments, clearSegments } = useStatusBar();

function updateStatusBar() {
  if (!state.formData.serialPort) return;
  setSegments([
    {
      label: `${state.formData.serialPort}:`,
      tag: {
        text: isOpen.value ? t('common.open') : t('common.close'),
        type: isOpen.value ? 'success' : 'danger',
      },
    },
    { label: `${t('serial.receive')}${receiveLength.value} ${t('common.bytes')}` },
    { label: `${t('serial.sendBytes')}${sendLength.value} ${t('common.bytes')}` },
  ]);
}

const vForm = ref();
const serialPortSetting = ref();
const receiveTextarea = ref<HTMLTextAreaElement>();

const state = reactive({
  formData: {
    serialPort: "",
    baudRate: 9600,
    autoSend: false,
    autoSendTimes: 1000,
    sendFormat: 0,
    sendContent: "",
    receiveFormat: 0,
    receiveContent: "",
    showSend: false,
  } as SerialPort,
  serialPortOptions: [] as Option[],
  baudRateOptions: [
    { label: "9600",   value: 9600   },
    { label: "19200",  value: 19200  },
    { label: "38400",  value: 38400  },
    { label: "57600",  value: 57600  },
    { label: "115200", value: 115200 },
  ],
  sendFormatOptions:    [{ label: "HEX", value: 0 }, { label: "ASCII", value: 1 }],
  receiveFormatOptions: [{ label: "HEX", value: 0 }, { label: "ASCII", value: 1 }],
})

let sendIntervalId = 0;
const isOpen = ref(false);
const openedPortName = ref("");
const isSend = ref(false);
const receiveLength = ref(0);
const sendLength = ref(0);
const unlisten = ref();

const open = async () => {
  if (!state.formData.serialPort) {
    ElMessage.error(t('serial.selectPort'));
    return;
  }
  if (!state.formData.baudRate) {
    ElMessage.error(t('serial.selectBaudRate'));
    return;
  }

  if (openedPortName.value && openedPortName.value !== state.formData.serialPort) {
    await invoke("stop_serial_port", {portName: openedPortName.value});
    if (unlisten.value) unlisten.value();
    openedPortName.value = "";
  }

  try {
    const result = await invoke<string>("open_serial_port", {portName: state.formData.serialPort, baudRate: state.formData.baudRate});
    if (result != "Opened") {
      ElMessage.error(result);
      return;
    }
    await cleanReturn();
    if (unlisten.value) unlisten.value();
    await read();
    ElMessage.success(t('serial.opened'));
  } catch (e) {
    ElMessage.error(t('serial.openFailed') + e);
  }
}

const send = async () => {
  if (!state.formData.sendContent.trim()) {
    ElMessage.warning(t('serial.inputSendContent'));
    return;
  }
  if (sendIntervalId != 0) {
    clearInterval(sendIntervalId);
    sendIntervalId = 0;
  }
  try {
    await invoke("write_to_serial_port", {
      portName: state.formData.serialPort,
      content: state.formData.sendContent.trim(),
      sendFormat: state.formData.sendFormat
    });
  } catch (e) {
    ElMessage.error(t('serial.sendFailed') + e);
  }
}

const audoSend = async () => {
  if (!state.formData.sendContent.trim()) {
    ElMessage.warning(t('serial.inputSendContent'));
    return;
  }
  if (sendIntervalId != 0) {
    clearInterval(sendIntervalId);
    sendIntervalId = 0;
  }
  try {
    sendIntervalId = setInterval(async () => {
      try {
        await invoke("write_to_serial_port", {
          portName: state.formData.serialPort,
          content: state.formData.sendContent.trim(),
          sendFormat: state.formData.sendFormat
        });
      } catch (e) {
        ElMessage.error(t('serial.sendFailed') + e);
        isSend.value = false;
      }
    }, state.formData.autoSendTimes);
    isSend.value = true;
  } catch (e) {
    ElMessage.error(t('serial.sendFailed') + e);
    isSend.value = false;
  }
}

const stop = async () => {
  if (sendIntervalId != 0) {
    clearInterval(sendIntervalId);
    sendIntervalId = 0;
  }
  if (unlisten.value) unlisten.value();
  await invoke("stop_serial_port", {portName: state.formData.serialPort})
  await cleanReturn()
}

const getPortList = async () => {
  let serialPortOptions = await invoke<String[]>("get_serial_port_list");
  state.serialPortOptions = serialPortOptions.map(i => ({ label: i, value: i } as Option));
}

const openSetting = () => {
  serialPortSetting.value.dialogVisible = true;
}

const cleanReturn = async () => {
  state.formData.receiveContent = "";
  receiveLength.value = 0;
  sendLength.value = 0;
  isOpen.value = await invoke<boolean>("is_serial_port_open", {portName: state.formData.serialPort});
  openedPortName.value = isOpen.value ? state.formData.serialPort : "";
}

const read = async () => {
  unlisten.value = await listen<SerialPortLog>('serial_port_log', (event) => {
    const { direction, content_hex, content_ascii, timestamp } = event.payload;
    const content = state.formData.receiveFormat === 0 ? content_hex : content_ascii;
    const arrow = direction === "TX" ? "->" : "<-";

    state.formData.receiveContent += `${timestamp} ${direction} ${arrow} ${content}\n`;

    const lines = state.formData.receiveContent.split('\n');
    if (lines.length > 200) {
      state.formData.receiveContent = lines.slice(-100).join('\n');
    }

    if (direction === "RX") {
      receiveLength.value += content_hex.split(' ').filter(Boolean).length;
    } else {
      sendLength.value += content_hex.split(' ').filter(Boolean).length;
    }

    nextTick(() => {
      if (receiveTextarea.value) {
        const textarea = (receiveTextarea.value as any)?.$el?.querySelector('textarea') || receiveTextarea.value;
        if (textarea) textarea.scrollTop = textarea.scrollHeight;
      }
    });
  });
}

watch([() => state.formData.serialPort, isOpen, receiveLength, sendLength], updateStatusBar);

onMounted(() => { getPortList(); })
onUnmounted(() => { stop(); clearSegments(); })
</script>

<template>
  <el-form :model="state.formData" ref="vForm" label-position="right" label-width="100px"
           @submit.prevent :inline="true" size="small">
    <el-form-item :label="t('serial.port')">
      <el-select v-model="state.formData.serialPort" clearable style="width: 250px" :disabled="isOpen">
        <el-option v-for="(item, index) in state.serialPortOptions" :key="index" :label="item.label" :value="item.value" />
      </el-select>
      <el-button @click="getPortList" :disabled="isOpen">{{ t('common.refresh') }}</el-button>
    </el-form-item>

    <el-form-item :label="t('serial.baudRate')">
      <el-select v-model="state.formData.baudRate" clearable style="width: 100px" :disabled="isOpen">
        <el-option v-for="(item, index) in state.baudRateOptions" :key="index" :label="item.label" :value="item.value" />
      </el-select>
      <el-button @click="openSetting" :disabled="isOpen">{{ t('common.settings') }}</el-button>
    </el-form-item>

    <el-form-item>
      <el-button type="primary" @click="open" v-if="!isOpen">{{ t('common.open') }}</el-button>
      <el-button type="danger" @click="stop" v-else>{{ t('common.close') }}</el-button>
    </el-form-item>
  </el-form>

  <el-form :model="state.formData" label-position="right" label-width="100px" size="small">
    <el-form-item :label="t('serial.sendSettings')">
      <el-radio-group v-model="state.formData.sendFormat">
        <el-radio v-for="item in state.sendFormatOptions" :value="item.value">{{ item.label }}</el-radio>
      </el-radio-group>
      &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;
      <el-input-number v-model="state.formData.autoSendTimes" :min="200" :max="1000000" :precision="0" :step="1" controls-position="right" />
      &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;
      <el-button type="primary" @click="send" v-if="isOpen">{{ t('common.send') }}</el-button>
      <el-button type="primary" @click="audoSend" v-if="isOpen" :disabled="isSend">{{ t('serial.pollingSend') }}</el-button>
    </el-form-item>

    <el-form-item :label="t('serial.sendContent')">
      <el-input type="textarea" v-model="state.formData.sendContent" :rows="4" />
    </el-form-item>

    <el-form-item :label="t('serial.receiveSettings')">
      <el-radio-group v-model="state.formData.receiveFormat">
        <el-radio v-for="item in state.receiveFormatOptions" :value="item.value">{{ item.label }}</el-radio>
      </el-radio-group>
      &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;
      <el-button @click="cleanReturn">{{ t('common.clear') }}</el-button>
    </el-form-item>

    <el-form-item :label="t('serial.receiveContent')">
      <el-input type="textarea" ref="receiveTextarea" v-model="state.formData.receiveContent" :rows="22" />
    </el-form-item>
  </el-form>

  <SerialPortSetting ref="serialPortSetting" />
</template>
