<script setup lang="ts">
import {ref, reactive, onMounted} from "vue";
import { invoke } from "@tauri-apps/api/tauri";
import SerialPortSetting from "./SerialPortSetting.vue";
import {SerialPort, Option} from "../types/serial";
import dayjs from 'dayjs';
import { ElMessage } from 'element-plus';

const vForm = ref();
const serialPortSetting = ref();
let intervalId = 0;

const state = reactive({
  formData: {
    serialPort: "",
    baudRate: 9600,
    autoSend: false,
    autoSendTimes: 1000,
    sendFormat: 0,
    sendContent: "",
    returnContent: "",
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
})

const send = async () => {
  if (!state.formData.serialPort || state.formData.serialPort == "") {
    ElMessage.error("请选择串口！");
    return;
  }
  if (!state.formData.baudRate) {
    ElMessage.error("请选择波特率！");
    return;
  }

  await invoke("open_serial_port", {portName: state.formData.serialPort, baudRate: state.formData.baudRate});
  const writeSuccess = await invoke<Boolean>("write_to_serial_port", {portName: state.formData.serialPort, content: state.formData.sendContent});
  if (writeSuccess) {
    if (intervalId != 0) {
      clearInterval(intervalId);
    }
    intervalId = setInterval(read, 200);
  }
}

const stop = async () => {
  if (intervalId != 0) {
    clearInterval(intervalId);
  }
}

const read = async ()=> {
  const res = await invoke<Uint8Array>("read_from_serial_port", {portName: state.formData.serialPort});
  if (res.length == 0) return;

  let time = "";
  if (state.formData.showTime) {
    time = dayjs().format("YYYY-MM-DD HH:mm:ss.SSS") + "[" + res.length + "]: ";
  }
  state.formData.returnContent += time + toHexString(res) + "\n";
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

onMounted(() => {
  getPortList();
})

function toHexString(byteArray: Uint8Array) {
  return Array.from(byteArray, function(byte) {
    return ('0' + (byte & 0xFF).toString(16)).slice(-2);
  }).join('')
}
</script>

<template>
  <el-form :model="state.formData" ref="vForm" label-position="right" label-width="80px"
           @submit.prevent :inline="true" size="small">
    <el-form-item label="串口:">
      <el-select v-model="state.formData.serialPort" clearable style="width: 200px">
        <el-option v-for="(item, index) in state.serialPortOptions" :key="index" :label="item.label"
                   :value="item.value"></el-option>
      </el-select>
      <el-button @click="getPortList">刷新</el-button>
    </el-form-item>

    <el-form-item label="波特率:">
      <el-select v-model="state.formData.baudRate" clearable style="width: 200px">
        <el-option v-for="(item, index) in state.baudRateOptions" :key="index" :label="item.label"
                   :value="item.value"></el-option>
      </el-select>
      <el-button @click="openSetting">设置</el-button>
    </el-form-item>

    <el-form-item>
      <el-button type="danger" @click="stop">停止</el-button>
      <el-button type="primary" @click="send">发送</el-button>
    </el-form-item>

    <el-form-item label="自动重发">
      <el-switch v-model="state.formData.autoSend"></el-switch>
      <el-input-number v-model="state.formData.autoSendTimes"
                       controls-position="right" :min="100" :max="1000000" :precision="0" :step="1">
      </el-input-number>
    </el-form-item>
  </el-form>

  <el-form :model="state.formData" label-position="right" label-width="80px" size="small">
    <el-form-item label="发送格式">
      <el-radio-group v-model="state.formData.sendFormat">
        <el-radio v-for="(item, index) in state.sendFormatOptions" :key="index" :label="item.value">{{item.label}}</el-radio>
      </el-radio-group>
    </el-form-item>
    <el-form-item label="发送内容">
      <el-input type="textarea" v-model="state.formData.sendContent" rows="4"></el-input>
    </el-form-item>

    <el-form-item label="接收设置">
      <el-radio-group v-model="state.formData.sendFormat">
        <el-radio v-for="(item, index) in state.sendFormatOptions" :key="index" :label="item.value">{{item.label}}</el-radio>
      </el-radio-group>
      &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;
      <el-checkbox v-model="state.formData.showTime" label="显示时间" value="true" />
    </el-form-item>
    <el-form-item label="接收内容">
      <el-input type="textarea" v-model="state.formData.returnContent" rows="23"></el-input>
    </el-form-item>
  </el-form>
  <SerialPortSetting ref="serialPortSetting"></SerialPortSetting>
</template>