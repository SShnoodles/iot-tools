<script setup lang="ts">
import {ref} from "vue";
import SerialPort from "./components/SerialPort.vue";
import Modbus from "./components/Modbus.vue";

const activeTab = ref("serialPort");
</script>

<template>
  <div class="container">
    <el-tabs v-model="activeTab" type="border-card">
      <el-tab-pane name="serialPort" label="串口调试">
        <SerialPort />
      </el-tab-pane>
      <el-tab-pane name="modbus" label="Modbus调试">
        <Modbus />
      </el-tab-pane>
    </el-tabs>
  </div>
</template>

<style scoped>

</style>
