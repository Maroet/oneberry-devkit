<template>
  <div class="dashboard fade-in">
    <!-- Header Area matching the image -->
    <div class="workspace-header">
      <div class="header-left">
        <h2 class="workspace-title">
          <span class="status-dot large-dot" :class="isEnvConnected ? 'green pulse' : 'gray'"></span>
          流量拦截
        </h2>
        

      </div>

      <div class="header-actions">
        <n-button 
          color="#0d9488" 
          class="intercept-btn" 
          @click="openServiceSelector"
          :disabled="!isEnvConnected"
        >
          <template #icon>
            <n-icon :component="Plus" />
          </template>
          拦截服务
        </n-button>
      </div>
    </div>

    <!-- Empty State -->
    <div class="empty-state fade-in-up" v-if="store.sessions.length === 0">
      <div class="empty-illustration">
        <n-icon :component="Rocket" size="48" color="#d1d5db" />
      </div>
      <h3 style="margin-top: 20px; color: var(--text-primary)">暂无活跃的拦截会话</h3>
      <p style="color: var(--text-secondary); margin-top: 8px;">已连接到开发集群。点击「拦截服务」将集群流量路由到你的本地机器。</p>
      
      <n-button 
        v-if="isEnvConnected"
        color="#0d9488" 
        ghost 
        size="large" 
        style="margin-top: 24px;"
        @click="openServiceSelector"
      >
        开始拦截
      </n-button>
    </div>

    <!-- Active Sessions List -->
    <div class="sessions-list fade-in-up" v-else>
      <div
        v-for="session in store.sessions"
        :key="session.id"
        class="session-card"
      >
        <!-- Card Main Row -->
        <div class="card-row align-top">
          <!-- Left: Identity -->
          <div class="service-identity">
            <span class="status-dot" :class="session.status === 'running' ? 'green' : 'yellow'"></span>
            <div class="service-details">
              <h4 class="service-name">{{ session.service }}</h4>
              <span class="service-port">端口: {{ session.service === 'oneberry-gateway' ? '80/443' : '3000' }}</span>
            </div>
          </div>

          <!-- Middle: Route -->
          <div class="route-info">
             <div class="route-arrows">
                <span class="arrow">→</span><span class="arrow">←</span>
             </div>
             <div class="route-target">
                <div class="target-address">
                  localhost:<span class="port-num">{{ session.port }}</span>
                </div>
                <div class="target-meta">
                  <span class="meta-label">本地端点</span>
                  <span class="mode-badge" :class="session.mode">{{ session.mode }}</span>
                </div>
             </div>
          </div>

          <!-- Right: Actions -->
          <div class="card-actions">
            <n-button size="small" secondary class="action-btn" @click="editSession(session)">
              <template #icon><n-icon :component="Pencil" /></template> 编辑
            </n-button>
            <n-button size="small" secondary class="action-btn" @click="toggleLog(session.id)">
               日志
            </n-button>
            <template v-if="session.status === 'stopped'">
              <n-button size="small" type="primary" class="action-btn reconnect-btn" :loading="reconnectingId === session.id" @click="reconnectSession(session)">
                <template #icon><n-icon :component="RefreshCw" /></template> 重新连接
              </n-button>
              <n-button size="small" secondary class="action-btn" @click="removeSession(session.id)">
                 清除
              </n-button>
            </template>
            <n-button v-else size="small" secondary class="action-btn" @click="store.stopSession(session.id)">
               停止
            </n-button>
          </div>
        </div>

        <!-- Card Bottom Row: Metadata -->
        <div class="card-meta-row">
          <div class="meta-group">
            <span class="meta-title">状态</span>
            <span class="meta-value" :class="session.status">{{ session.status === 'running' ? '运行中' : session.status === 'stopped' ? '已停止' : '启动中' }}</span>
          </div>
          <div class="meta-group">
            <span class="meta-title">运行时长</span>
            <span class="meta-value">{{ formatUptime(session.started_at) }}</span>
          </div>
          <div class="meta-group" v-if="session.mode === 'mesh' && session.version_header">
            <span class="meta-title">Version Header</span>
            <span class="meta-value mono">{{ session.version_header }}</span>
          </div>
        </div>

        <!-- Inline Log Panel -->
        <div v-if="expandedSession === session.id" class="inline-log-panel">
          <div class="log-content" :ref="setLogRef(session.id)">
            <div v-for="(log, i) in getLogs(session.id)" :key="i" class="log-line" :class="log.stream">
              <span class="log-time">{{ formatTime(log.timestamp) }}</span>
              <span class="log-text">{{ stripAnsi(log.line) }}</span>
            </div>
            <div v-if="getLogs(session.id).length === 0" class="log-empty">
              等待日志输出...
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Service Selection Modal -->
    <n-modal v-model:show="showServiceSelector" preset="card" title="选择要拦截的服务" class="service-modal" style="width: 500px; border-radius: 16px;">
      <div class="modal-search">
        <n-input v-model:value="searchQuery" placeholder="搜索服务名称..." clearable>
          <template #prefix><n-icon :component="Search" /></template>
        </n-input>
        <n-button @click="store.refreshServices()" :loading="store.loading" secondary>刷新</n-button>
      </div>

      <div class="service-table-container">
        <n-spin :show="store.loading" description="加载服务列表...">
          <n-data-table
            :columns="columns"
            :data="filteredServices"
            :bordered="false"
            size="small"
            :max-height="300"
          />
        </n-spin>
      </div>
    </n-modal>

    <!-- Configure Intercept Modal -->
    <n-modal v-model:show="showExchange" preset="card" :title="`拦截配置: ${selectedService}`" class="config-modal" style="width: 460px; border-radius: 16px;">
      <n-form label-placement="left" label-width="100">
        <n-form-item label="模式">
          <n-radio-group v-model:value="exchangeMode">
            <n-space vertical>
              <n-radio value="exchange">
                <strong>Exchange</strong> — 将所有流量路由到本地。
              </n-radio>
              <n-radio value="mesh">
                <strong>Mesh</strong> — 仅当请求包含特定 Header 时路由到本地。
              </n-radio>
            </n-space>
          </n-radio-group>
        </n-form-item>
        <n-form-item label="本地端口">
          <n-input-number v-model:value="localPort" :min="1" :max="65535" style="width: 100%" />
        </n-form-item>
        <n-form-item v-if="exchangeMode === 'mesh'" label="Version Header">
          <n-input v-model:value="meshVersionHeader" placeholder="例如: devkit-abc123" />
          <template #feedback>
            <span style="font-size: 12px; color: var(--text-muted);">请求头中 <code>X-Version</code> 值匹配时，流量将路由到本地</span>
          </template>
        </n-form-item>
      </n-form>
      <template #action>
        <n-space justify="end">
          <n-button @click="showExchange = false">取消</n-button>
          <n-button type="primary" @click="doExchange" :loading="exchangeLoading">
            <template #icon><n-icon :component="Rocket" /></template>
            开始拦截
          </n-button>
        </n-space>
      </template>
    </n-modal>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, h } from 'vue'
import { NButton, NTag, NSpace, NInput, NSpin, useMessage, useDialog, NIcon } from 'naive-ui'
import { Rocket, Plus, Pencil, Search, RefreshCw } from 'lucide-vue-next'
import { useAppStore, type SessionLogLine } from '../stores/app'

const store = useAppStore()
const message = useMessage()
const dialog = useDialog()

// Environment state
const isEnvConnected = computed(() => store.vpn.status === 'connected' && store.cluster.status === 'connected')

// Service Selector Modal
const showServiceSelector = ref(false)
const searchQuery = ref('')
const filteredServices = computed(() => {
  if (!searchQuery.value) return store.services
  const q = searchQuery.value.toLowerCase()
  return store.services.filter(s => s.name.toLowerCase().includes(q))
})

// Config Modal
const showExchange = ref(false)
const selectedService = ref('')
const exchangeMode = ref('exchange')
const localPort = ref(38000)
const exchangeLoading = ref(false)
const meshVersionHeader = ref('')
const editingSessionId = ref<string | null>(null)
const reconnectingId = ref<string | null>(null)

// Logs
const expandedSession = ref<string | null>(null)
const logContainerRefs = ref<Record<string, HTMLElement | null>>({})

function setLogRef(id: string) {
  return (el: any) => {
    logContainerRefs.value[id] = el as HTMLElement
  }
}

function getLogs(id: string) {
  return store.getSessionLogs(id)
}

function toggleLog(sessionId: string) {
  expandedSession.value = expandedSession.value === sessionId ? null : sessionId
}

function formatTime(ts: string) {
  try {
    return new Date(ts).toLocaleTimeString('zh-CN', { hour12: false })
  } catch {
    return ''
  }
}

// Strip ANSI escape codes (the □ squares in logs)
function stripAnsi(str: string): string {
  // eslint-disable-next-line no-control-regex
  return str.replace(/\x1b\[[0-9;]*m/g, '')
}

function formatUptime(startedAt: string) {
  try {
    const start = new Date(startedAt).getTime()
    const now = Date.now()
    const diff = Math.floor((now - start) / 1000)
    if (diff < 60) return `${diff}s`
    if (diff < 3600) return `${Math.floor(diff / 60)}m`
    const h = Math.floor(diff / 3600)
    const m = Math.floor((diff % 3600) / 60)
    return `${h}h ${m}m`
  } catch {
    return '--'
  }
}

function editSession(session: any) {
  editingSessionId.value = session.id
  selectedService.value = session.service
  exchangeMode.value = session.mode
  localPort.value = session.port
  meshVersionHeader.value = session.version_header || ''
  showExchange.value = true
}

function openServiceSelector() {
  store.refreshServices()
  searchQuery.value = ''
  showServiceSelector.value = true
}

// Table columns for the Service Selector
const columns = [
  { title: '服务', key: 'name', width: 220, ellipsis: { tooltip: true } },
  {
    title: '状态', key: 'status', width: 100,
    render: (row: any) => h(NTag, {
      type: row.status === 'running' ? 'success' : 'warning',
      size: 'small', bordered: false, round: true,
    }, () => `${row.ready}/${row.desired}`)
  },
  {
    title: '操作', key: 'action', width: 80,
    render: (row: any) => h(NButton, {
      size: 'small', type: 'primary',
      onClick: () => openConfigModal(row.name),
    }, () => '选择')
  },
]

function openConfigModal(service: string) {
  showServiceSelector.value = false
  selectedService.value = service
  exchangeMode.value = 'exchange'
  editingSessionId.value = null
  meshVersionHeader.value = ''
  
  // Auto-fill common ports
  const portMap: Record<string, number> = {
    'oneberry-gateway': 38000,
    'oneberry-api': 31001,
    'oneberry-auth': 31002,
    'oneberry-system': 31003,
    'oneberry-bop': 31004,
    'oneberry-cop': 31005,
    'oneberry-ai': 31006,
    'oneberry-job': 31007,
  }
  localPort.value = portMap[service] || 38000
  showExchange.value = true
}

async function doExchange() {
  exchangeLoading.value = true
  try {
    if (exchangeMode.value === 'exchange') {
      const session = await store.startExchange(selectedService.value, localPort.value)
      message.success(`Exchange 已启动: ${selectedService.value} → localhost:${localPort.value}`)
      expandedSession.value = session.id
    } else {
      const session = await store.startMesh(selectedService.value, localPort.value)
      message.success(`Mesh 已启动，查看日志获取 Version Header`)
      expandedSession.value = session.id
    }
    showExchange.value = false
  } catch (e: any) {
    const errMsg = typeof e === 'string' ? e : (e?.message || '启动失败')
    // Detect stale session conflicts: "already exchanging", kt-selector annotation, invalid status
    const isConflict = ['already exchanging', 'already', 'kt-selector', 'invalid status'].some(kw => errMsg.includes(kw))
    if (isConflict) {
      dialog.warning({
        title: '服务冲突',
        content: `「${selectedService.value}」存在残留的拦截状态。\n\n可能是你上次异常退出的残留，也可能是同事正在联调。\n\n强制接管会清理残留并重新发起拦截。`,
        positiveText: '强制接管',
        negativeText: '取消',
        onPositiveClick: async () => {
          try {
            message.loading('正在清理残留会话...')
            await store.recoverService(selectedService.value)
            message.success('清理完成，正在重新拦截...')
            // Auto-retry after cleanup
            await doExchange()
          } catch (recoverErr: any) {
            message.error(`清理失败: ${recoverErr}`)
          }
        },
      })
    } else {
      message.error(errMsg)
    }
  } finally {
    exchangeLoading.value = false
  }
}

async function reconnectSession(session: any) {
  reconnectingId.value = session.id
  try {
    // Remove old dead session first
    store.sessions = store.sessions.filter(s => s.id !== session.id)
    store.sessionLogs.delete(session.id)

    // Start a new session with the same parameters
    let newSession
    if (session.mode === 'mesh') {
      newSession = await store.startMesh(session.service, session.port)
      message.success(`Mesh 已重新连接: ${session.service}`)
    } else {
      newSession = await store.startExchange(session.service, session.port)
      message.success(`Exchange 已重新连接: ${session.service} → localhost:${session.port}`)
    }
    expandedSession.value = newSession.id
  } catch (e: any) {
    const errMsg = typeof e === 'string' ? e : (e?.message || '重新连接失败')
    message.error(errMsg)
  } finally {
    reconnectingId.value = null
  }
}

function removeSession(sessionId: string) {
  store.sessions = store.sessions.filter(s => s.id !== sessionId)
  store.sessionLogs.delete(sessionId)
  if (expandedSession.value === sessionId) expandedSession.value = null
}

// Event listeners
let cleanups: (() => void)[] = []

onMounted(async () => {
  await store.refreshSessions()

  if (!store.mockMode) {
    try {
      const { listen } = await import('@tauri-apps/api/event')

      // Session log events
      cleanups.push(await listen<SessionLogLine>('session:log', (event) => {
        store.addLogLine(event.payload)
        // Auto-scroll disabled per user request
        // if (expandedSession.value === event.payload.session_id) {
        //   scrollLogToBottom(event.payload.session_id)
        // }
      }))

      // Session ended events
      cleanups.push(await listen<string>('session:ended', (event) => {
        store.markSessionEnded(event.payload)
      }))
    } catch {}
  }
})

onUnmounted(() => cleanups.forEach(fn => fn()))
</script>

<style scoped>
.dashboard {
  max-width: 900px;
  margin: 0 auto;
  padding-bottom: 40px;
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
  gap: 24px;
}

.workspace-title {
  font-size: 24px;
  font-weight: 700;
  margin: 0;
  color: var(--text-primary);
}

.filter-tabs {
  display: flex;
  gap: 16px;
  border-bottom: 1px solid transparent;
}

.tab {
  font-size: 14px;
  color: var(--text-secondary);
  padding: 4px 0;
  cursor: pointer;
  border-bottom: 2px solid transparent;
  transition: all 0.2s;
}

.tab:hover {
  color: var(--text-primary);
}

.tab.active {
  color: #0d9488;
  font-weight: 600;
  border-bottom-color: #0d9488;
}

.intercept-btn {
  border-radius: 6px;
  font-weight: 600;
}

/* Sessions List (1 per row) */
.sessions-list {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.session-card {
  background: var(--bg-card);
  border: 1px solid var(--border-color);
  border-left: 3px solid #0d9488;
  border-radius: 12px;
  padding: 20px;
  display: flex;
  flex-direction: column;
  transition: all 0.2s;
  overflow: hidden;
}

.session-card:hover {
  box-shadow: 0 4px 12px rgba(0,0,0,0.05);
  border-color: rgba(0,0,0,0.1);
}

.card-row {
  display: flex;
  justify-content: space-between;
}

.align-top { align-items: flex-start; }

.service-identity {
  display: flex;
  gap: 12px;
  width: 250px;
}

.service-identity .status-dot {
  margin-top: 6px;
  flex-shrink: 0;
}

.service-details .service-name {
  font-size: 16px;
  font-weight: 600;
  margin: 0 0 4px 0;
  color: var(--text-primary);
}

.service-details .service-port {
  font-size: 13px;
  color: var(--text-secondary);
}

.route-info {
  display: flex;
  align-items: flex-start;
  gap: 16px;
  flex: 1;
}

.route-arrows {
  display: flex;
  flex-direction: column;
  color: var(--text-muted);
  font-size: 14px;
  line-height: 1;
  margin-top: 2px;
}

.route-target {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.target-address {
  font-size: 15px;
  font-family: var(--font-mono);
  color: var(--text-primary);
}

.port-num {
  font-weight: 700;
}

.target-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
}

.meta-label {
  color: var(--text-secondary);
}

.mode-badge {
  font-size: 10px;
  padding: 2px 6px;
  border-radius: 4px;
  font-weight: 600;
}
.mode-badge.exchange { background: rgba(99, 102, 241, 0.1); color: var(--accent); }
.mode-badge.mesh { background: rgba(13, 148, 136, 0.1); color: #0d9488; }

.card-actions {
  display: flex;
  gap: 8px;
}

.action-btn {
  border-radius: 6px;
}

.reconnect-btn {
  font-weight: 600;
}

/* Bottom Meta Row */
.card-meta-row {
  display: flex;
  align-items: flex-end;
  gap: 32px;
  margin-top: 16px;
  padding-top: 16px;
  border-top: 1px solid rgba(0,0,0,0.03);
}

.meta-group {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.meta-title {
  font-size: 12px;
  color: var(--text-secondary);
}

.meta-value {
  font-size: 13px;
  font-weight: 500;
  color: var(--text-primary);
}

.meta-value.running { color: #0d9488; }
.meta-value.stopped { color: #ef4444; }
.meta-value.mono { font-family: var(--font-mono); font-size: 12px; }

.modal-search {
  display: flex;
  gap: 12px;
  margin-bottom: 16px;
}

/* Inline Logs */
.inline-log-panel {
  margin: 16px -20px -20px -20px;
  border-top: 1px solid var(--border-color);
  background: #1e1e2e;
  border-radius: 0 0 12px 12px;
  overflow: hidden;
}

.log-content {
  height: 200px;
  overflow-y: auto;
  padding: 12px 0;
  font-family: var(--font-mono);
  font-size: 12px;
  line-height: 1.5;
  user-select: text;
  -webkit-user-select: text;
}

.log-content::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.15);
}

.log-line {
  padding: 2px 16px;
  display: flex;
  gap: 12px;
}

.log-line:hover { background: rgba(255, 255, 255, 0.03); }

.log-line.stdout { color: #a6e3a1; }
.log-line.stderr { color: #f38ba8; }

.log-time { color: #585b70; flex-shrink: 0; }
.log-text { word-break: break-all; }

.log-empty {
  padding: 20px;
  text-align: center;
  color: #585b70;
  font-style: italic;
}

.log-hint {
  margin-top: 8px;
  font-size: 11px;
  color: #45475a;
  font-style: normal;
}

@keyframes pulse {
  0% { opacity: 1; }
  50% { opacity: 0.5; }
  100% { opacity: 1; }
}

@keyframes slideArrows {
  0% { transform: translateX(-4px); }
  100% { transform: translateX(8px); }
}
</style>
