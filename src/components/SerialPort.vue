<script setup lang="ts">
import {ref, reactive, onMounted} from "vue";
import { invoke } from "@tauri-apps/api/tauri";
import SerialPortSetting from "./SerialPortSetting.vue";
import {SerialPort, Option} from "../types/serial";
import * as dayjs from 'dayjs';
import { ElMessage } from 'element-plus';

const vForm = ref();
const serialPortSetting = ref();

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
    showTime: false,
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
let receiveIntervalId = 0;
const isOpen = ref(false);
const isSend = ref(false);
const receiveLength = ref(0);
const sendLength = ref(0);

const open = async () => {
  if (!state.formData.serialPort || state.formData.serialPort == "") {
    ElMessage.error("请选择串口！");
    return;
  }
  if (!state.formData.baudRate) {
    ElMessage.error("请选择波特率！");
    return;
  }
  invoke<String>("open_serial_port", {portName: state.formData.serialPort, baudRate: state.formData.baudRate})
      .then(() => cleanReturn())
      .catch(e => ElMessage.error(e));
}

const send = async () => {
  let bytes: number[];
  if (state.formData.sendFormat == 0) {
    bytes = hexToBytes(state.formData.sendContent.trim());
  } else {
    bytes = asciiToBytes(state.formData.sendContent.trim());
  }
  sendLength.value = bytes.length;

  if (sendIntervalId != 0) {
    clearInterval(sendIntervalId);
    sendIntervalId = 0;
  }
  const writeSuccess = await invoke<Boolean>("write_to_serial_port", {portName: state.formData.serialPort, content: bytes});
  if (writeSuccess) {
    if (receiveIntervalId != 0) {
      clearInterval(receiveIntervalId);
      receiveIntervalId = 0;
    }
    receiveIntervalId = setInterval(read, 100);

    // if auto send then start interval
    if (state.formData.autoSend) {
      sendIntervalId = setInterval(() => {
        invoke<Boolean>("write_to_serial_port", {portName: state.formData.serialPort, content: bytes})
      }, state.formData.autoSendTimes);
    }
  }
}

const stop = async () => {
  if (receiveIntervalId != 0) {
    clearInterval(receiveIntervalId);
    receiveIntervalId = 0;
  }
  if (sendIntervalId != 0) {
    clearInterval(sendIntervalId);
    sendIntervalId = 0;
  }
  await invoke("stop_serial_port", {portName: state.formData.serialPort})
  await cleanReturn()
}

const read = async ()=> {
  let res = await invoke<number[]>("read_from_serial_port", {portName: state.formData.serialPort});
  receiveLength.value = res.length;
  if (res.length == 0) return;

  let time = "";
  if (state.formData.showTime) {
    time = dayjs().format("YYYY-MM-DD HH:mm:ss.SSS") + "[" + res.length + "]: ";
  }
  let content: string;
  if (state.formData.receiveFormat == 0) {
    content = bytesToHex(res);
  } else {
    content = bytesToAscii(res);
  }
  state.formData.receiveContent += time + content + "\n";
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
  isOpen.value = await invoke<boolean>("is_serial_port_open", {portName: state.formData.serialPort});
}

onMounted(() => {
  getPortList();
})

function bytesToHex(bytes: number[]) {
  return bytes.map(byte => byte.toString(16).padStart(2, "0")).join(" ");
}

function bytesToAscii(bytes: number[]): string {
  return bytes.map(byte => String.fromCharCode(byte)).join("");
}

function asciiToBytes(asciiText: string) {
  asciiText = asciiText.replace(/\s+/g, "");
  let bytes = [];
  for (let i = 0; i < asciiText.length; ++i) {
    const charCode = asciiText.charCodeAt(i);
    bytes.push(charCode);
  }
  return bytes;
}

function hexToBytes(hexText: string) {
  hexText = hexText.replace(/\s+/g, "");
  let bytes = [];
  for (let i = 0; i < hexText.length; i += 2) {
    const byte = parseInt(hexText.substr(i, 2), 16);
    bytes.push(byte);
  }
  return bytes;
}
</script>

<template>
  <el-form :model="state.formData" ref="vForm" label-position="right" label-width="80px"
           @submit.prevent :inline="true" size="small">
    <el-form-item label="串口:">
      <el-select v-model="state.formData.serialPort" clearable style="width: 250px">
        <el-option v-for="(item, index) in state.serialPortOptions" :key="index" :label="item.label"
                   :value="item.value"></el-option>
      </el-select>
      <el-button @click="getPortList">刷新</el-button>
    </el-form-item>

    <el-form-item label="波特率:">
      <el-select v-model="state.formData.baudRate" clearable style="width: 100px">
        <el-option v-for="(item, index) in state.baudRateOptions" :key="index" :label="item.label"
                   :value="item.value"></el-option>
      </el-select>
      <el-button @click="openSetting">设置</el-button>
    </el-form-item>

    <el-form-item>
      <el-button type="danger" @click="stop">停止</el-button>
      <el-button type="primary" @click="send" v-if="isOpen" :disabled="isSend">发送</el-button>
      <el-button type="primary" @click="open" v-else>打开</el-button>
    </el-form-item>

  </el-form>

  <el-form :model="state.formData" label-position="right" label-width="80px" size="small">
    <el-form-item label="发送设置">
      <el-radio-group v-model="state.formData.sendFormat">
        <el-radio v-for="(item, index) in state.sendFormatOptions" :key="index" :label="item.value">{{item.label}}</el-radio>
      </el-radio-group>
      &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;
      <el-text>自动重发</el-text>
      <el-switch v-model="state.formData.autoSend"></el-switch>
      <el-input-number v-model="state.formData.autoSendTimes" :min="200" :max="1000000" :precision="0" :step="1" controls-position="right">
      </el-input-number>
    </el-form-item>

    <el-form-item label="发送内容">
      <el-input type="textarea" v-model="state.formData.sendContent" rows="4"></el-input>
    </el-form-item>

    <el-form-item label="接收设置">
      <el-radio-group v-model="state.formData.receiveFormat">
        <el-radio v-for="(item, index) in state.receiveFormatOptions" :key="index" :label="item.value">{{item.label}}</el-radio>
      </el-radio-group>
      &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;
      <el-checkbox v-model="state.formData.showTime" label="显示时间" value="true" />
      &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;
      <el-button @click="cleanReturn">清空</el-button>
    </el-form-item>
    <el-form-item label="接收内容">
      <el-input type="textarea" v-model="state.formData.receiveContent" rows="22"></el-input>
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