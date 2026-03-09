<script setup lang="ts">
import { useStatusBar } from '../composables/useStatusBar'
const { segments } = useStatusBar()
</script>

<template>
  <div class="status-bar" v-if="segments.length > 0">
    <template v-for="(seg, idx) in segments" :key="idx">
      <span v-if="idx > 0" class="status-sep">|</span>
      <el-text size="small">
        {{ seg.label }}
        <el-tag v-if="seg.tag" :type="seg.tag.type" size="small" style="margin-left: 4px;">
          {{ seg.tag.text }}
        </el-tag>
        <template v-for="et in seg.extraTags" :key="et.text">
          <el-tag :type="et.type" size="small" style="margin-left: 4px;">{{ et.text }}</el-tag>
        </template>
      </el-text>
    </template>
  </div>
</template>

<style scoped>
.status-bar {
  position: fixed;
  bottom: 0;
  left: 0;
  right: 0;
  height: 28px;
  display: flex;
  align-items: center;
  padding: 0 12px;
  gap: 8px;
  background-color: var(--el-bg-color-overlay);
  border-top: 1px solid var(--el-border-color);
  z-index: 100;
}
.status-sep {
  color: var(--el-border-color-darker);
  user-select: none;
}
</style>
