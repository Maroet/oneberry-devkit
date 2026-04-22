<template>
  <n-config-provider :locale="zhCN" :date-locale="dateZhCN">
    <n-message-provider>
      <n-notification-provider>
        <n-dialog-provider>
        <div class="app-container">
          
          <!-- Sidebar -->
          <aside class="sidebar">
            <div class="sidebar-header" @click="router.push('/')">
              <n-icon size="24" :component="Hexagon" color="var(--primary-color, #0d9488)" />
              <h1>OneBerry</h1>
            </div>
            
            <nav class="sidebar-nav">
              <a 
                class="nav-item" 
                :class="{ active: route.name === 'dashboard' }" 
                @click="router.push('/')"
              >
                <n-icon size="18" :component="LayoutDashboard" />
                Dashboard
              </a>
              <a 
                class="nav-item" 
                :class="{ active: route.name === 'settings' }" 
                @click="router.push('/settings')"
              >
                <n-icon size="18" :component="SettingsIcon" />
                Settings
              </a>
              <a 
                class="nav-item" 
                :class="{ active: route.name === 'logs' }" 
                @click="router.push('/logs')"
              >
                <n-icon size="18" :component="ScrollText" />
                Logs
              </a>
            </nav>

            <div class="sidebar-footer">
               <div class="indicator" title="VPN Status">
                <span class="status-dot" :class="vpnDotClass"></span>
                <span>VPN {{ store.vpn.status === 'connected' ? 'ON' : 'OFF' }}</span>
              </div>
              <div class="indicator mt-2" title="Cluster Status">
                <span class="status-dot" :class="clusterDotClass"></span>
                <span>Cluster {{ store.cluster.status === 'connected' ? 'ON' : 'OFF' }}</span>
              </div>
              <div class="version-badge mt-4" style="display: flex; flex-direction: column; gap: 4px;">
                <span v-if="store.mockMode" class="badge mock" style="display: flex; align-items: center; gap: 4px;"><n-icon :component="Beaker" /> Mock Mode</span>
                <span>v0.1.0</span>
              </div>
            </div>
          </aside>

          <!-- Main Layout Area -->
          <div class="main-panel">
            <header class="app-header">
              <div class="header-left">
                <span class="env-label">开发环境</span>
                <n-switch
                  class="switch-control"
                  :value="isEnvConnected"
                  @update:value="toggleEnvironment"
                  :loading="isConnecting"
                  size="large"
                >
                  <template #checked>ON</template>
                  <template #unchecked>OFF</template>
                </n-switch>
                <span class="status-text" :class="envStatusClass">{{ envStatusText }}</span>
              </div>

              <div class="header-right">
                <!-- Placeholder for future right aligned items like user profile -->
              </div>
            </header>
            
            <main class="app-content">
              <router-view />
            </main>
        </div>

        </div>
        </n-dialog-provider>
      </n-notification-provider>
    </n-message-provider>
  </n-config-provider>
</template>

<script setup lang="ts">
import { zhCN, dateZhCN, createDiscreteApi, NIcon } from 'naive-ui'
import { Hexagon, LayoutDashboard, Settings as SettingsIcon, Beaker, ScrollText } from 'lucide-vue-next'
import { useRouter, useRoute } from 'vue-router'
import { computed, ref, onMounted } from 'vue'
import { openUrl } from '@tauri-apps/plugin-opener'
import { useAppStore } from './stores/app'

const { message, dialog } = createDiscreteApi(['message', 'dialog'])

const router = useRouter()
const route = useRoute()
const store = useAppStore()

const isConnecting = ref(false)

// Check environment status on startup + periodic refresh
onMounted(async () => {
  await store.refreshVpn()
  await store.refreshCluster()

  // Keep status in sync — poll every 15s
  setInterval(async () => {
    await store.refreshVpn()
    await store.refreshCluster()
  }, 15000)
})

const isEnvConnected = computed(() => {
  return store.vpn.status === 'connected' && store.cluster.status === 'connected'
})

const envStatusClass = computed(() => {
  if (isConnecting.value) return 'warning'
  if (isEnvConnected.value) return 'success'
  return 'muted'
})

const envStatusText = computed(() => {
  if (store.vpn.status === 'not_installed') return '未安装'
  if (isConnecting.value) return '连接中...'
  if (isEnvConnected.value) return '已连接'
  return '未连接'
})

const vpnDotClass = computed(() =>
  store.vpn.status === 'connected' ? 'green' :
  store.vpn.status === 'disconnected' ? 'yellow' : 'gray'
)

const clusterDotClass = computed(() =>
  store.cluster.status === 'connected' ? 'green' : 'red'
)

async function toggleEnvironment(val: boolean) {
  if (val) {
    // If VPN already connected, just sync cluster and return
    if (store.vpn.status === 'connected') {
      isConnecting.value = true
      store.addSystemLog('-> VPN already connected, syncing cluster...')
      await store.refreshCluster()
      isConnecting.value = false
      if (store.cluster.status === 'connected') {
        message.success('环境已就绪')
      }
      return
    }
    
    // Always refresh status first so we have accurate state
    isConnecting.value = true
    store.addSystemLog('-> Checking environment...')
    await store.refreshVpn()
    store.addSystemLog(`[DEBUG] vpn.status=${store.vpn.status}`)
    
    // After refresh, check if Tailscale is installed
    if (store.vpn.status === 'not_installed') {
      isConnecting.value = false
      store.addSystemLog('[DEBUG] -> Showing install dialog')
      dialog.info({
        title: '需要初始化环境',
        content: '检测到运行环境缺少底层网络组件 (Tailscale)。只需点击"立即安装"，DevKit 会自动完成授权和部署。',
        positiveText: '🚀 立即安装',
        negativeText: '取消',
        onPositiveClick: async () => {
          isConnecting.value = true
          store.addSystemLog('-> 正在静默安装系统组件...')
          try {
            const installResult = await store.installTailscale()
            store.addSystemLog(`-> install result: ${installResult}`)
            // Refresh status after install
            await store.refreshVpn()
            store.addSystemLog(`[DEBUG] post-install vpn.status=${store.vpn.status}`)
            
            if (store.vpn.status === 'not_installed') {
              // Daemon not ready yet — may need Network Extension approval
              message.warning(installResult || '安装似乎未完成，请检查系统设置')
              isConnecting.value = false
            } else {
              message.success('组件安装成功！正在连接...')
              store.addSystemLog(`[DEBUG] -> Auto-proceeding to connect (status=${store.vpn.status})`)
              // Auto-proceed to connect
              setTimeout(() => toggleEnvironment(true), 1000)
            }
          } catch (e: any) {
            store.addSystemLog(`-> 安装失败: ${e}`)
            message.error(`安装失败: ${e}`)
            isConnecting.value = false
          }
        }
      })
      return
    }

    // Already installed, proceed to connect
    store.addSystemLog(`-> Init VPN Connection (current status=${store.vpn.status})`)
    
    let authOpened = false
    
    // connectVpn now uses spawn() and returns quickly with status info
    try {
      const res = await store.connectVpn()
      store.addSystemLog(`VPN Cmd: ${res}`)
      
      // If the backend already detected an auth URL, open it immediately
      if (res && res.startsWith('AUTH_REQUIRED:')) {
        const authUrl = res.substring('AUTH_REQUIRED:'.length)
        store.addSystemLog(`-> Auth required. Opening browser...`)
        authOpened = true
        try {
          await openUrl(authUrl)
        } catch (e) {
          store.addSystemLog(`Failed to open browser: ${e}`)
        }
      }
    } catch (e) {
      store.addSystemLog(`VPN Error: ${e}`)
      isConnecting.value = false
      return
    }
    
    let attempts = 0
    const poll = setInterval(async () => {
      attempts++
      store.addSystemLog(`Polling Status (Attempt ${attempts})...`)
      await store.refreshVpn()
      
      if (store.vpn.status === 'connected') {
        clearInterval(poll)
        store.addSystemLog('-> VPN Connected, syncing cluster...')
        await store.refreshCluster()
        isConnecting.value = false
        message.success('Connected to Dev Environment')
        store.addSystemLog('-> Sync Complete')
      }
      
      if ((store.vpn.status === 'needs_login' || store.vpn.status === 'needs_auth') && store.vpn.auth_url) {
        if (!authOpened) {
          authOpened = true
          store.addSystemLog(`-> Auth required. Opening browser...`)
          try {
            await openUrl(store.vpn.auth_url)
          } catch (e) {
            store.addSystemLog(`Failed to open browser: ${e}`)
          }
        }
      }

      if (attempts > 30) { 
        clearInterval(poll)
        isConnecting.value = false
        store.addSystemLog('Timeout waiting for connection.')
        message.error('Connection timeout')
      }
    }, 2000)
  } else {
    store.addSystemLog('-> Disconnecting VPN')
    try {
      isConnecting.value = true
      await store.disconnectVpn()
      // Poll for disconnected status instead of fixed wait
      let attempts = 0
      const poll = setInterval(async () => {
        attempts++
        await store.refreshVpn()
        if (store.vpn.status !== 'connected') {
          clearInterval(poll)
          await store.refreshCluster()
          isConnecting.value = false
          store.addSystemLog('-> Disconnected')
          message.success('Disconnected.')
        } else if (attempts > 15) {
          clearInterval(poll)
          isConnecting.value = false
          store.addSystemLog('-> Disconnect timeout')
          message.warning('断开超时，请稍后重试')
        }
      }, 1000)
    } catch (e: any) {
      isConnecting.value = false
      store.addSystemLog(`Disconnect Error: ${e}`)
      message.error(e.toString())
    }
  }
}
</script>

<style scoped>
.app-container {
  display: flex;
  height: 100vh;
  width: 100vw;
  background-color: var(--bg-body);
  overflow: hidden;
}

/* Sidebar Styles */
.sidebar {
  width: 240px;
  background: #ffffff;
  border-right: 1px solid var(--border-color);
  display: flex;
  flex-direction: column;
  -webkit-app-region: drag;
  z-index: 10;
}

.sidebar-header {
  padding: 24px 20px;
  display: flex;
  align-items: center;
  gap: 12px;
  cursor: pointer;
  -webkit-app-region: no-drag;
}

.sidebar-header .logo {
  font-size: 24px;
}

.sidebar-header h1 {
  font-size: 18px;
  font-weight: 700;
  color: var(--text-primary);
  margin: 0;
  letter-spacing: -0.02em;
}

.sidebar-nav {
  display: flex;
  flex-direction: column;
  padding: 10px 12px;
  flex: 1;
  -webkit-app-region: no-drag;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 16px;
  margin-bottom: 4px;
  border-radius: 8px;
  color: var(--text-secondary);
  font-weight: 500;
  font-size: 14px;
  cursor: pointer;
  transition: all 0.2s;
}

.nav-item:hover {
  background: var(--bg-card);
  color: var(--text-primary);
}

.nav-item.active {
  background: rgba(16, 185, 129, 0.1);
  color: var(--success);
  font-weight: 600;
}

.sidebar-footer {
  padding: 24px 20px;
  border-top: 1px solid rgba(0,0,0,0.04);
  font-size: 12px;
  color: var(--text-muted);
  -webkit-app-region: no-drag;
}

.indicator {
  display: flex;
  align-items: center;
  gap: 8px;
}

.mt-2 { margin-top: 8px; }
.mt-4 { margin-top: 16px; }

/* Main Panel */
.main-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background-color: var(--bg-body);
}

.app-header {
  height: 72px;
  padding: 0 32px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--border-color);
  background: rgba(255, 255, 255, 0.8);
  backdrop-filter: blur(20px);
  -webkit-backdrop-filter: blur(20px);
  -webkit-app-region: drag;
  z-index: 5;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 16px;
  -webkit-app-region: no-drag;
}

.env-label {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary);
}

.switch-control {
  margin-left: 4px;
}

.status-text {
  font-size: 13px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.status-text.muted { color: var(--text-muted); }
.status-text.warning { color: var(--warning); }
.status-text.success { color: var(--success); }

.app-content {
  flex: 1;
  overflow-y: auto;
  padding: 32px;
  -webkit-app-region: no-drag;
}

.status-dot {
  display: inline-block;
  border-radius: 50%;
  width: 8px;
  height: 8px;
}
.status-dot.green { background: var(--success); box-shadow: 0 0 6px var(--success); }
.status-dot.yellow { background: var(--warning); }
.status-dot.red { background: var(--error); box-shadow: 0 0 6px var(--error); }
.status-dot.gray { background: var(--text-muted); }
</style>