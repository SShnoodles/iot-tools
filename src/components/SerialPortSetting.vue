<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { invoke } from '@tauri-apps/api/core';

const { t } = useI18n();

const dialogVisible = ref(false);

const form = ref({
  dataBits: 8,
  stopBits: 1,
  parity: "None",
  flowControl: "None",
});

const dataBitsOptions = [
  {label: "5", value: 5},
  {label: "6", value: 6},
  {label: "7", value: 7},
  {label: "8", value: 8},
]
const stopBitsOptions = [
  {label: "1", value: 1},
  {label: "2", value: 2},
]
const parityOptions = [
  {label: "None", value: "None"},
  {label: "Even", value: "Even"},
  {label: "Odd", value: "Odd"},
]
const flowControlOptions = [
  {label: "None", value: "None"},
  {label: "Software", value: "Software"},
  {label: "Hardware", value: "Hardware"},
]

const updateConfig = async () => {
  await invoke("set_serial_port_config", form.value);
  dialogVisible.value = false;
}

defineExpose({
  dialogVisible,
  form
});
</script>

<template>
  <el-dialog v-model="dialogVisible" :title="t('serial.portSettings')" width="500">
    <el-form label-position="right" label-width="auto" :model="form" style="max-width: 600px">
      <el-form-item :label="t('serial.dataBits')">
        <el-select v-model="form.dataBits" style="width: 240px">
          <el-option v-for="item in dataBitsOptions" :key="item.value" :label="item.label" :value="item.value" />
        </el-select>
      </el-form-item>
      <el-form-item :label="t('serial.stopBits')">
        <el-select v-model="form.stopBits" style="width: 240px">
          <el-option v-for="item in stopBitsOptions" :key="item.value" :label="item.label" :value="item.value" />
        </el-select>
      </el-form-item>
      <el-form-item :label="t('serial.parity')">
        <el-select v-model="form.parity" style="width: 240px">
          <el-option v-for="item in parityOptions" :key="item.value" :label="item.label" :value="item.value" />
        </el-select>
      </el-form-item>
      <el-form-item :label="t('serial.flowControl')">
        <el-select v-model="form.flowControl" style="width: 240px">
          <el-option v-for="item in flowControlOptions" :key="item.value" :label="item.label" :value="item.value" />
        </el-select>
      </el-form-item>
    </el-form>
    <template #footer>
      <div class="dialog-footer">
        <el-button @click="dialogVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button type="primary" @click="updateConfig">{{ t('common.confirm') }}</el-button>
      </div>
    </template>
  </el-dialog>
</template>
