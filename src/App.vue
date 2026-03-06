<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { useI18n } from "vue-i18n";
import zhCn from "element-plus/es/locale/lang/zh-cn";
import en from "element-plus/es/locale/lang/en";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import SerialPort from "./components/SerialPort.vue";
import Modbus from "./components/Modbus.vue";

const { t, locale } = useI18n();

const activeTab = ref("serialPort");

const elLocale = computed(() => (locale.value === "zh" ? zhCn : en));

let unlisten: UnlistenFn | undefined;

onMounted(async () => {
  unlisten = await listen<string>("lang-change", (event) => {
    locale.value = event.payload;
  });
});

onUnmounted(() => {
  unlisten?.();
});
</script>

<template>
  <el-config-provider :locale="elLocale">
    <div class="container">
      <el-tabs v-model="activeTab" type="border-card">
        <el-tab-pane name="serialPort" :label="t('app.serialPort')">
          <SerialPort v-if="activeTab === 'serialPort'" />
        </el-tab-pane>
        <el-tab-pane name="modbus" :label="t('app.modbus')">
          <Modbus v-if="activeTab === 'modbus'" />
        </el-tab-pane>
      </el-tabs>
    </div>
  </el-config-provider>
</template>
