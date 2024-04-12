<script setup lang="ts">
import {ref, reactive, onMounted} from "vue";
import { invoke } from "@tauri-apps/api/tauri";
import SerialPortSetting from "./SerialPortSetting.vue";
import {SerialPort, Option} from "../types/serial";

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
    returnContent: ""
  } as SerialPort,
  rules: {
    
  },
  'serialPortTabActiveTab': 'tab1',
  'returnTabActiveTab': 'tab1',
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
    "label": "文本",
    "value": 1
  }],
})

const send = async () => {
  let valid = await vForm.value.validate();
  if (!valid) return;

  await invoke("open_serial_port", {portName: state.formData.serialPort, baudRate: state.formData.baudRate});
  const writeSuccess = await invoke<Boolean>("write_to_serial_port", {portName: state.formData.serialPort, content: state.formData.sendContent});
  if (writeSuccess) {
    const res = await invoke<Uint8Array>("read_from_serial_port", {portName: state.formData.serialPort});
    state.formData.returnContent = toHexString(res);
  }
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
  <el-form :model="state.formData" ref="vForm" :rules="state.rules" label-position="right" label-width="80px"
           size="default" @submit.prevent>
    <div class="tab-container">
      <el-tabs v-model="state.serialPortTabActiveTab" type="border-card">
        <el-tab-pane name="tab1" label="串口调试">
          <el-row>
            <el-col :span="7" class="grid-cell">
              <el-form-item label="串口:" prop="serialPort" class="label-right-align">
                <el-select v-model="state.formData.serialPort" class="full-width-input" clearable>
                  <el-option v-for="(item, index) in state.serialPortOptions" :key="index" :label="item.label"
                             :value="item.value"></el-option>
                </el-select>
              </el-form-item>
            </el-col>
            <el-col :span="1" class="grid-cell">
              <div class="static-content-item">
                <el-button @click="getPortList">刷新</el-button>
              </div>
            </el-col>
            <el-col :span="5" class="grid-cell">
              <el-form-item label="波特率:" prop="baudRate" class="label-right-align">
                <el-select v-model="state.formData.baudRate" class="full-width-input" clearable>
                  <el-option v-for="(item, index) in state.baudRateOptions" :key="index" :label="item.label"
                             :value="item.value"></el-option>
                </el-select>
              </el-form-item>
            </el-col>
            <el-col :span="3" class="grid-cell">
              <div class="static-content-item">
                <el-button @click="openSetting">设置</el-button>
              </div>
            </el-col>
            <el-col :span="1" class="grid-cell">
              <div class="static-content-item">
                <el-button type="primary" @click="send">发送</el-button>
              </div>
            </el-col>
            <el-col :span="5" class="grid-cell">
              <el-form-item label="自动重发" prop="autoSend">
                <el-switch v-model="state.formData.autoSend"></el-switch>
                <el-input-number v-model="state.formData.autoSendTimes" class="full-width-input"
                                 controls-position="right" :min="100" :max="1000000" :precision="0" :step="1">
                </el-input-number>
              </el-form-item>
            </el-col>
          </el-row>
        </el-tab-pane>
      </el-tabs>
    </div>
    <el-form-item label="发送格式" prop="sendFormat">
      <el-radio-group v-model="state.formData.sendFormat">
        <el-radio v-for="(item, index) in state.sendFormatOptions" :key="index" :label="item.value">{{item.label}}</el-radio>
      </el-radio-group>
    </el-form-item>
    <el-form-item label="发送内容" prop="sendContent">
      <el-input type="textarea" v-model="state.formData.sendContent" rows="8"></el-input>
    </el-form-item>
    <div class="static-content-item">
      <el-divider direction="horizontal">返回内容</el-divider>
    </div>
    <div class="tab-container">
      <el-tabs v-model="state.returnTabActiveTab" type="border-card">
        <el-tab-pane name="tab1" label="HEX">
          {{state.formData.returnContent}}
        </el-tab-pane>
        <el-tab-pane name="tab-pane-52597" label="文本">
          {{state.formData.returnContent}}
        </el-tab-pane>
      </el-tabs>
    </div>
  </el-form>
  <SerialPortSetting ref="serialPortSetting"></SerialPortSetting>
</template>