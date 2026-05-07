<template>
  <div class="settings fade-in">
    <div class="settings-container">
      <n-form label-placement="left" label-width="120" :model="config">
        <!-- Network -->
        <div class="card settings-section fade-in-up stagger-1">
          <div class="section-title">
            <span>🌐</span>
            <h3>网络</h3>
          </div>
          <n-form-item label="Headscale">
            <n-input v-model:value="config.headscale_url" placeholder="https://vpn.oneberry.cc:31443" />
          </n-form-item>
          <n-form-item label="VPN 状态">
            <n-tag :type="vpnConnected ? 'success' : 'error'" size="small" round :bordered="false">
              {{ vpnConnected ? `已连接 (${store.vpn.ip})` : '未连接' }}
            </n-tag>
          </n-form-item>
        </div>

        <!-- Cluster -->
        <div class="card settings-section fade-in-up stagger-2">
          <div class="section-title">
            <span>☸️</span>
            <h3>集群</h3>
          </div>
          <n-form-item label="命名空间">
            <n-input v-model:value="config.namespace" />
          </n-form-item>
        </div>

        <!-- KtConnect -->
        <div class="card settings-section fade-in-up stagger-3">
          <div class="section-title">
            <span>🔧</span>
            <h3>KtConnect</h3>
          </div>
          <n-form-item label="Shadow Node">
            <n-input v-model:value="config.shadow_node" />
          </n-form-item>
          <n-form-item label="Shadow Image">
            <n-input v-model:value="config.shadow_image" />
          </n-form-item>
        </div>

        <!-- Bundled Tools -->
        <div class="card settings-section fade-in-up stagger-4">
          <div class="section-title">
            <span>📦</span>
            <h3>内嵌工具</h3>
          </div>
          <div class="tool-list">
            <div class="tool-item">
              <span class="tool-name">kubectl</span>
              <n-tag size="tiny" type="success" :bordered="false" round>v1.32 · 内置</n-tag>
            </div>
            <div class="tool-item">
              <span class="tool-name">ktctl</span>
              <n-tag size="tiny" type="success" :bordered="false" round>v0.3.7 · 内置</n-tag>
            </div>
            <div class="tool-item">
              <span class="tool-name">Tailscale</span>
              <n-tag size="tiny" :type="vpnConnected ? 'success' : 'warning'" :bordered="false" round>
                {{ vpnConnected ? '已安装' : '需安装' }}
              </n-tag>
            </div>
          </div>
        </div>

        <!-- Software Update -->
        <div class="card settings-section fade-in-up stagger-5">
          <div class="section-title">
            <span>⬆️</span>
            <h3>软件更新</h3>
          </div>
          <div class="update-section">
            <div class="update-row">
              <span class="update-label">当前版本</span>
              <n-tag size="small" :bordered="false" round>v{{ appVersion }}</n-tag>
            </div>
            <div v-if="updater.state.value === 'available'" class="update-row">
              <span class="update-label">最新版本</span>
              <n-tag size="small" type="success" :bordered="false" round>v{{ updater.newVersion.value }}</n-tag>
            </div>
            <div class="update-row">
              <span class="update-label">更新状态</span>
              <span class="update-status">
                <template v-if="updater.state.value === 'idle'">✅ 已是最新版本</template>
                <template v-else-if="updater.state.value === 'checking'">… 正在检查</template>
                <template v-else-if="updater.state.value === 'available'">🆕 v{{ updater.newVersion.value }} 可用</template>
                <template v-else-if="updater.state.value === 'downloading'">⬇️ 下载中...</template>
                <template v-else-if="updater.state.value === 'installing'">📦 安装中...</template>
                <template v-else-if="updater.state.value === 'error'">❌ {{ updater.errorMsg.value }}</template>
              </span>
            </div>
            <div class="update-actions">
              <n-button
                size="small"
                :loading="updater.state.value === 'checking'"
                :disabled="updater.state.value === 'downloading' || updater.state.value === 'installing'"
                @click="updater.checkForUpdates()"
              >
                🔍 检查更新
              </n-button>
              <n-button
                v-if="updater.state.value === 'available'"
                size="small"
                type="primary"
                @click="updater.installUpdate()"
              >
                ⬆️ 立即更新
              </n-button>
            </div>
          </div>
        </div>

        <div style="display:flex;gap:12px;justify-content:flex-end;margin-top:16px;">
          <n-button @click="resetConfig" quaternary>恢复默认</n-button>
          <n-button type="primary" @click="saveSettings" :loading="saving">
            💾 保存
          </n-button>
        </div>
      </n-form>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, reactive } from 'vue'
import { useMessage } from 'naive-ui'
import { useAppStore } from '../stores/app'
import { useUpdater } from '../composables/useUpdater'

const message = useMessage()
const store = useAppStore()
const updater = useUpdater()
const saving = ref(false)
const appVersion = ref('0.0.0')

// 获取当前应用版本
try {
  const isTauri = !!(window as any).__TAURI_INTERNALS__
  if (isTauri) {
    import('@tauri-apps/api/app').then(({ getVersion }) => {
      getVersion().then(v => { appVersion.value = v })
    })
  }
} catch {}

const config = reactive({
  headscale_url: 'https://vpn.oneberry.cc:31443',
  namespace: 'oneberry-dev',
  shadow_node: 'hmdev-node01',
  shadow_image: 'image.hm.metavarse.tech:9443/hongmei-dev/kt-connect-shadow:v0.3.7',
  theme: 'system',
})

const vpnConnected = computed(() => store.vpn.status === 'connected')

async function saveSettings() {
  saving.value = true
  try {
    const isTauri = !!(window as any).__TAURI_INTERNALS__
    if (isTauri) {
      const { invoke } = await import('@tauri-apps/api/core')
      await invoke('save_config', { config })
    }
    message.success('设置已保存')
  } catch (e: any) {
    message.error(typeof e === 'string' ? e : '保存失败')
  } finally {
    saving.value = false
  }
}
function resetConfig() {
  config.headscale_url = 'https://vpn.oneberry.cc:31443'
  config.namespace = 'oneberry-dev'
  config.shadow_node = 'hmdev-node01'
  config.shadow_image = 'image.hm.metavarse.tech:9443/hongmei-dev/kt-connect-shadow:v0.3.7'
  config.theme = 'system'
}

onMounted(async () => {
  try {
    const isTauri = !!(window as any).__TAURI_INTERNALS__
    if (isTauri) {
      const { invoke } = await import('@tauri-apps/api/core')
      const saved = await invoke<any>('get_config')
      Object.assign(config, saved)
    }
  } catch {}
  store.refreshVpn()
})
</script>

<style scoped>
.settings {
  max-width: 620px;
  margin: 0 auto;
}

.settings-section {
  margin-bottom: 14px;
}

.section-title {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 16px;
  padding-bottom: 10px;
  border-bottom: 1px solid var(--border-color);
}

.section-title span {
  font-size: 16px;
}

.section-title h3 {
  font-size: 13px;
  font-weight: 600;
}

.tool-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.tool-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 0;
  font-size: 13px;
}

.tool-name {
  font-family: var(--font-mono);
  font-weight: 500;
}

.update-section {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.update-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 13px;
}

.update-label {
  color: var(--text-secondary);
  font-weight: 500;
}

.update-status {
  font-size: 13px;
  color: var(--text-primary);
}

.update-actions {
  display: flex;
  gap: 8px;
  margin-top: 4px;
}
</style>
