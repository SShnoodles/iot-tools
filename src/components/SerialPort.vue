<script setup lang="ts">
import { ref, reactive, onMounted, onUnmounted, nextTick } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from '@tauri-apps/api/event';
import SerialPortSetting from "./SerialPortSetting.vue";
import { SerialPort, Option, SerialPortLog } from "../types/serial";
import { ElMessage } from 'element-plus';

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
  baudRateOptions: [{
    "label": "9600",
    "value": 9600
  }, {
    "label": "19200",
    "value": 19200
  }, {
    "label": "38400",
    "value": 38400
  }, {
    "label": "57600",
    "value": 57600
  }, {
    "label": "115200",
    "value": 115200
  }],
  sendFormatOptions: [{
    "label": "HEX",
    "value": 0
  }, {
    "label": "ASCII",
    "value": 1
  }],
  receiveFormatOptions: [{
    "label": "HEX",
    "value": 0
  }, {
    "label": "ASCII",
    "value": 1
  }],
})

let sendIntervalId = 0;
const isOpen = ref(false);
const openedPortName = ref("");
const isSend = ref(false);
const receiveLength = ref(0);
const sendLength = ref(0);
const unlisten = ref();

const open = async () => {
  if (!state.formData.serialPort || state.formData.serialPort == "") {
    ElMessage.error("请选择串口！");
    return;
  }
  if (!state.formData.baudRate) {
    ElMessage.error("请选择波特率！");
    return;
  }

  // 如果后端还有其他端口未关闭，先关闭它（防御性处理）
  if (openedPortName.value && openedPortName.value !== state.formData.serialPort) {
    await invoke("stop_serial_port", {portName: openedPortName.value});
    if (unlisten.value) {
      unlisten.value();
    }
    openedPortName.value = "";
  }

  try {
    const result = await invoke<string>("open_serial_port", {portName: state.formData.serialPort, baudRate: state.formData.baudRate});
    if (result != "Opened") {
      ElMessage.error(result);
      return;
    }
    await cleanReturn();

    if (unlisten.value) {
      unlisten.value();
    }
    await read();
    ElMessage.success("串口已打开");
  } catch (e) {
    ElMessage.error("打开串口失败: " + e);
  }
}

const send = async () => {
  if (!state.formData.sendContent.trim()) {
    ElMessage.warning("请输入发送内容");
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
    ElMessage.error("发送失败: " + e);
  }
}

const audoSend = async () => {
  if (!state.formData.sendContent.trim()) {
    ElMessage.warning("请输入发送内容");
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
        ElMessage.error("发送失败: " + e);
        isSend.value = false;
      }
    }, state.formData.autoSendTimes);
    isSend.value = true;
  } catch (e) {
    ElMessage.error("发送失败: " + e);
    isSend.value = false;
  }
}

const stop = async () => {
  if (sendIntervalId != 0) {
    clearInterval(sendIntervalId);
    sendIntervalId = 0;
  }
  if (unlisten.value) {
    unlisten.value();
  }
  await invoke("stop_serial_port", {portName: state.formData.serialPort})
  await cleanReturn()
}

const getPortList = async () => {
  let serialPortOptions = await invoke<String[]>("get_serial_port_list");
  state.serialPortOptions = serialPortOptions.map(i => {
    return {"label": i, "value": i} as Option;
  })
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

    // 超过 200 行时保留后 100 行，避免内存无限增长
    const lines = state.formData.receiveContent.split('\n');
    if (lines.length > 200) {
      state.formData.receiveContent = lines.slice(-100).join('\n');
    }

    // 统计字节数（hex 格式下每组"xx"是1字节，用空格分隔）
    if (direction === "RX") {
      receiveLength.value += content_hex.split(' ').filter(Boolean).length;
    } else {
      sendLength.value += content_hex.split(' ').filter(Boolean).length;
    }

    // 自动滚动到底部
    nextTick(() => {
      if (receiveTextarea.value) {
        const textarea = (receiveTextarea.value as any)?.$el?.querySelector('textarea') || receiveTextarea.value;
        if (textarea) {
          textarea.scrollTop = textarea.scrollHeight;
        }
      }
    });
  });
}

onMounted(() => {
  getPortList();
})

onUnmounted(() => {
  if (unlisten.value) {
    unlisten.value();
  }
})
</script>

<template>
  <el-form :model="state.formData" ref="vForm" label-position="right" label-width="80px"
           @submit.prevent :inline="true" size="small">
    <el-form-item label="串口:">
      <el-select v-model="state.formData.serialPort" clearable style="width: 250px" :disabled="isOpen">
        <el-option v-for="(item, index) in state.serialPortOptions" :key="index" :label="item.label"
                   :value="item.value"></el-option>
      </el-select>
      <el-button @click="getPortList" :disabled="isOpen">刷新</el-button>
    </el-form-item>

    <el-form-item label="波特率:">
      <el-select v-model="state.formData.baudRate" clearable style="width: 100px" :disabled="isOpen">
        <el-option v-for="(item, index) in state.baudRateOptions" :key="index" :label="item.label"
                   :value="item.value"></el-option>
      </el-select>
      <el-button @click="openSetting" :disabled="isOpen">设置</el-button>
    </el-form-item>

    <el-form-item>
      <el-button type="primary" @click="open" v-if="!isOpen">打开</el-button>
      <el-button type="primary" @click="stop" v-else>关闭</el-button>
    </el-form-item>

  </el-form>

  <el-form :model="state.formData" label-position="right" label-width="80px" size="small">
    <el-form-item label="发送设置">
      <el-radio-group v-model="state.formData.sendFormat">
        <el-radio v-for="item in state.sendFormatOptions" :value="item.value">{{item.label}}</el-radio>
      </el-radio-group>
      &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;
      <el-input-number v-model="state.formData.autoSendTimes" :min="200" :max="1000000" :precision="0" :step="1" controls-position="right">
      </el-input-number>
      &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;
      <el-button type="primary" @click="send" v-if="isOpen">发送</el-button>
      <el-button type="primary" @click="audoSend" v-if="isOpen" :disabled="isSend">轮询发送</el-button>
    </el-form-item>

    <el-form-item label="发送内容">
      <el-input type="textarea" v-model="state.formData.sendContent" :rows="4"></el-input>
    </el-form-item>

    <el-form-item label="接收设置">
      <el-radio-group v-model="state.formData.receiveFormat">
        <el-radio v-for="item in state.receiveFormatOptions" :value="item.value">{{item.label}}</el-radio>
      </el-radio-group>
      &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;
      <el-button @click="cleanReturn">清空</el-button>
    </el-form-item>
    <el-form-item label="接收内容">
      <el-input type="textarea" ref="receiveTextarea" v-model="state.formData.receiveContent" :rows="22"></el-input>
    </el-form-item>
  </el-form>
  <SerialPortSetting ref="serialPortSetting"></SerialPortSetting>

  <el-row v-if="state.formData.serialPort">
    <el-col :span="12">
      <el-text>{{state.formData.serialPort}}:
        <el-tag type="success" v-if="isOpen">打开</el-tag>
        <el-tag type="danger" v-else>关闭</el-tag>
      </el-text>
    </el-col>
    <el-col :span="4">
      <el-text>接收: {{receiveLength}} bytes</el-text>
    </el-col>
    <el-col :span="4">
      <el-text>发送: {{sendLength}} bytes</el-text>
    </el-col>
  </el-row>
</template>