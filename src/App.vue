<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { useI18n } from "vue-i18n";
import zhCn from "element-plus/es/locale/lang/zh-cn";
import en from "element-plus/es/locale/lang/en";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { ElMessage, ElNotification } from "element-plus";
import SerialPort from "./components/SerialPort.vue";
import Modbus from "./components/Modbus.vue";
import Mqtt from "./components/Mqtt.vue";
import OpcUa from "./components/OpcUa.vue";
import StatusBar from "./components/StatusBar.vue";

const { t, locale } = useI18n();

const activeTab = ref("serialPort");

const elLocale = computed(() => (locale.value === "zh" ? zhCn : en));

const unlisteners: UnlistenFn[] = [];

onMounted(async () => {
  unlisteners.push(await listen<string>("lang-change", (event) => {
    locale.value = event.payload;
  }));
  unlisteners.push(await listen<string>("update-available", (event) => {
    ElNotification({
      title: t('app.newVersionTitle'),
      message: t('app.newVersionMsg', { version: event.payload }),
      type: "warning",
      duration: 0,
    });
  }));
  unlisteners.push(await listen<string>("update-up-to-date", () => {
    ElMessage.success(t('app.upToDate'));
  }));
  unlisteners.push(await listen<string>("update-check-failed", (event) => {
    ElMessage.error(t('app.updateCheckFailed') + event.payload);
  }));
});

onUnmounted(() => {
  unlisteners.forEach(fn => fn());
});
</script>

<template>
  <el-config-provider :locale="elLocale">
    <div class="container" style="padding-bottom: 28px;">
      <el-tabs v-model="activeTab" type="border-card">
        <el-tab-pane name="serialPort" :label="t('app.serialPort')">
          <SerialPort v-if="activeTab === 'serialPort'" />
        </el-tab-pane>
        <el-tab-pane name="modbus" :label="t('app.modbus')">
          <Modbus v-if="activeTab === 'modbus'" />
        </el-tab-pane>
        <el-tab-pane name="mqtt" :label="t('app.mqtt')">
          <Mqtt v-if="activeTab === 'mqtt'" />
        </el-tab-pane>
        <el-tab-pane name="opcua" :label="t('app.opcua')">
          <OpcUa v-if="activeTab === 'opcua'" />
        </el-tab-pane>
      </el-tabs>
    </div>
    <StatusBar />
  </el-config-provider>
</template>
