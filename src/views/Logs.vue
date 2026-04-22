<template>
  <div class="logs-page fade-in">
    <div class="workspace-header">
      <div class="header-left">
        <h2 class="workspace-title">系统日志</h2>
        <span class="log-count">{{ store.systemLogs.length }} entries</span>
      </div>
      <div class="header-actions">
        <n-button secondary size="small" @click="copyLogs">
          <template #icon><n-icon :component="Copy" /></template>
          复制全部
        </n-button>
        <n-button secondary size="small" @click="scrollToBottom">
          <template #icon><n-icon :component="ArrowDownToLine" /></template>
          滚动到底部
        </n-button>
        <n-button secondary size="small" @click="clearLogs">
          <template #icon><n-icon :component="Trash2" /></template>
          清空
        </n-button>
      </div>
    </div>
    
    <div class="logs-container">
      <div class="log-scroll" ref="systemLogRef">
        <div v-for="(log, i) in store.systemLogs" :key="i" class="sys-log-line">
          {{ log }}
        </div>
        <div v-if="store.systemLogs.length === 0" class="sys-log-empty">
          等待系统事件...
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, nextTick, onMounted } from 'vue'
import { NButton, NIcon, useMessage } from 'naive-ui'
import { Trash2, Copy, ArrowDownToLine } from 'lucide-vue-next'
import { useAppStore } from '../stores/app'

const store = useAppStore()
const msg = useMessage()
const systemLogRef = ref<HTMLElement | null>(null)

// NO automatic scroll on new log entries — user controls scroll manually.

function scrollToBottom() {
  nextTick(() => {
    if (systemLogRef.value) {
      systemLogRef.value.scrollTop = systemLogRef.value.scrollHeight
    }
  })
}

onMounted(() => {
  scrollToBottom()
})

function clearLogs() {
  store.systemLogs.splice(0, store.systemLogs.length)
}

async function copyLogs() {
  const text = store.systemLogs.join('\n')
  try {
    await navigator.clipboard.writeText(text)
    msg.success('已复制到剪贴板')
  } catch {
    // Fallback for non-secure contexts
    const textarea = document.createElement('textarea')
    textarea.value = text
    document.body.appendChild(textarea)
    textarea.select()
    document.execCommand('copy')
    document.body.removeChild(textarea)
    msg.success('已复制到剪贴板')
  }
}
</script>

<style scoped>
.logs-page {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.workspace-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 24px;
  padding-bottom: 16px;
  border-bottom: 1px solid var(--border-color);
}

.header-left {
  display: flex;
  align-items: baseline;
  gap: 12px;
}

.workspace-title {
  font-size: 24px;
  font-weight: 700;
  margin: 0;
  color: var(--text-primary);
}

.log-count {
  font-size: 13px;
  color: var(--text-muted);
}

.header-actions {
  display: flex;
  gap: 8px;
}

.logs-container {
  flex: 1;
  background: #1e1e2e;
  border-radius: 12px;
  border: 1px solid rgba(0,0,0,0.1);
  overflow: hidden;
  display: flex;
  flex-direction: column;
  box-shadow: inset 0 2px 10px rgba(0,0,0,0.2);
}

.log-scroll {
  flex: 1;
  overflow-y: auto;
  padding: 20px;
  font-family: var(--font-mono);
  font-size: 13px;
  color: #cdd6f4;
  line-height: 1.6;
  /* Enable text selection */
  user-select: text;
  -webkit-user-select: text;
  cursor: text;
}

.sys-log-line {
  margin-bottom: 8px;
  word-break: break-all;
  border-bottom: 1px solid rgba(255,255,255,0.05);
  padding-bottom: 4px;
  /* Ensure selectable */
  user-select: text;
  -webkit-user-select: text;
}

.sys-log-line:hover {
  background: rgba(255,255,255,0.02);
}

.sys-log-empty {
  font-style: italic;
  opacity: 0.5;
  text-align: center;
  margin-top: 40px;
}
</style>
