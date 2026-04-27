import { defineStore } from 'pinia'
import { ref } from 'vue'

// Check if running inside Tauri
const isTauri = !!(window as any).__TAURI_INTERNALS__

async function safeInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri) {
    return getMockData<T>(cmd, args)
  }
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<T>(cmd, args)
}

// Mock data for browser development mode
function getMockData<T>(cmd: string, _args?: Record<string, unknown>): T {
  const mocks: Record<string, unknown> = {
    check_vpn: {
      status: 'connected',
      ip: '100.64.0.12',
      hostname: 'dev-mac-haoxiang',
    },
    check_cluster: {
      status: 'connected',
      node_count: 3,
      message: null,
    },
    list_services: [
      { name: 'oneberry-gateway', ready: 1, desired: 1, status: 'running' },
      { name: 'oneberry-auth', ready: 1, desired: 1, status: 'running' },
      { name: 'oneberry-system', ready: 1, desired: 1, status: 'running' },
      { name: 'oneberry-bop', ready: 0, desired: 1, status: 'stopped' },
      { name: 'oneberry-cop', ready: 1, desired: 2, status: 'degraded' },
      { name: 'oneberry-ai', ready: 1, desired: 1, status: 'running' },
      { name: 'oneberry-job', ready: 1, desired: 1, status: 'running' },
    ],
    check_setup: {
      tailscale_installed: true,
      kubectl_available: true,
      ktctl_available: true,
      daemon_running: false,
    },
    get_config: {
      headscale_url: 'https://vpn.oneberry.cc:31443',
      namespace: 'oneberry-dev',
      shadow_node: 'hmdev-node01',
      shadow_image: 'image.hm.metavarse.tech:9443/hongmei-dev/kt-connect-shadow:v0.3.7',
      theme: 'system',
    },
    save_config: '配置已保存',
    connect_vpn: 'VPN 连接请求已发送',
    disconnect_vpn: 'VPN 已断开',
    start_exchange: {
      id: 'exchange-mock-123456',
      service: 'oneberry-gateway',
      port: 38000,
      mode: 'exchange',
      started_at: new Date().toISOString(),
      version_header: null,
      status: 'running',
    },
    start_mesh: {
      id: 'mesh-mock-789012',
      service: 'oneberry-gateway',
      port: 38000,
      mode: 'mesh',
      started_at: new Date().toISOString(),
      version_header: 'devkit-789012',
      status: 'running',
    },
    list_sessions: [],
    stop_session: '会话已停止',
    install_tailscale: 'Tailscale 安装完成',
  }
  return (mocks[cmd] ?? null) as T
}

export interface VpnStatus {
  status: string
  auth_url?: string
  ip?: string
  hostname?: string
  debug_info?: string
}

export interface ClusterStatus {
  status: string
  node_count: number
  message?: string
}

export interface K8sService {
  name: string
  ready: number
  desired: number
  status: string
}

export interface SessionInfo {
  id: string
  service: string
  port: number
  mode: string
  started_at: string
  version_header?: string
  status: string
}

export interface SessionLogLine {
  session_id: string
  stream: 'stdout' | 'stderr'
  line: string
  timestamp: string
}

export interface SetupStatus {
  tailscale_installed: boolean
  kubectl_available: boolean
  ktctl_available: boolean
  daemon_running: boolean
}

export const useAppStore = defineStore('app', () => {
  const vpn = ref<VpnStatus>({ status: 'unknown' })
  const cluster = ref<ClusterStatus>({ status: 'unknown', node_count: 0 })
  const services = ref<K8sService[]>([])
  const sessions = ref<SessionInfo[]>([])
  const sessionLogs = ref<Map<string, SessionLogLine[]>>(new Map())
  const systemLogs = ref<string[]>([])
  const loading = ref(false)
  const mockMode = ref(!isTauri)

  function addSystemLog(msg: string) {
    const time = new Date().toLocaleTimeString('zh-CN', { hour12: false })
    systemLogs.value.push(`[${time}] ${msg}`)
    if (systemLogs.value.length > 100) systemLogs.value.shift()
  }

  async function checkSetup(): Promise<SetupStatus> {
    return await safeInvoke<SetupStatus>('check_setup')
  }

  async function refreshVpn() {
    try {
      vpn.value = await safeInvoke<VpnStatus>('check_vpn')
      if (vpn.value.debug_info) {
        addSystemLog(`[DEBUG] check_vpn: ${vpn.value.debug_info}`)
      }
    } catch (e) {
      addSystemLog(`[DEBUG] refreshVpn error: ${e}`)
      console.warn('refreshVpn failed:', e)
    }
  }

  async function connectVpn() {
    const result = await safeInvoke<string>('connect_vpn')
    setTimeout(() => refreshVpn(), 3000)
    return result
  }

  async function disconnectVpn() {
    await safeInvoke<string>('disconnect_vpn')
    setTimeout(() => refreshVpn(), 1000)
  }

  async function refreshCluster() {
    try {
      cluster.value = await safeInvoke<ClusterStatus>('check_cluster')
    } catch (e) {
      console.warn('refreshCluster failed:', e)
    }
  }

  async function refreshServices() {
    try {
      services.value = await safeInvoke<K8sService[]>('list_services')
    } catch (e) {
      console.warn('Failed to list services:', e)
    }
  }

  async function refreshSessions() {
    try {
      sessions.value = await safeInvoke<SessionInfo[]>('list_sessions')
    } catch (e) {
      console.warn('Failed to list sessions:', e)
    }
  }

  async function startExchange(service: string, port: number) {
    const session = await safeInvoke<SessionInfo>('start_exchange', { service, port })
    sessions.value.push(session)
    sessionLogs.value.set(session.id, [])
    return session
  }

  async function startMesh(service: string, port: number, versionHeader?: string) {
    const session = await safeInvoke<SessionInfo>('start_mesh', { service, port, versionHeader })
    sessions.value.push(session)
    sessionLogs.value.set(session.id, [])
    return session
  }

  async function stopSession(sessionId: string) {
    await safeInvoke<string>('stop_session', { sessionId })
    sessions.value = sessions.value.filter(s => s.id !== sessionId)
    sessionLogs.value.delete(sessionId)
  }

  function addLogLine(log: SessionLogLine) {
    const logs = sessionLogs.value.get(log.session_id) || []
    logs.push(log)
    // Keep last 500 lines per session
    if (logs.length > 500) logs.splice(0, logs.length - 500)
    sessionLogs.value.set(log.session_id, logs)
  }

  function getSessionLogs(sessionId: string): SessionLogLine[] {
    return sessionLogs.value.get(sessionId) || []
  }

  function markSessionEnded(sessionId: string) {
    const session = sessions.value.find(s => s.id === sessionId)
    if (session) {
      session.status = 'stopped'
    }
  }

  async function refreshAll() {
    loading.value = true
    await Promise.all([refreshVpn(), refreshCluster(), refreshServices()])
    loading.value = false
  }

  async function installTailscale() {
    return await safeInvoke<string>('install_tailscale')
  }

  async function recoverService(service: string) {
    return await safeInvoke<string>('recover_service', { service })
  }

  return {
    vpn, cluster,    services,
    sessions,
    sessionLogs,
    systemLogs,
    addSystemLog,
    loading, mockMode,
    checkSetup, refreshVpn, connectVpn, disconnectVpn, refreshCluster,
    refreshServices, refreshSessions, startExchange, startMesh, stopSession,
    addLogLine, getSessionLogs, markSessionEnded, refreshAll, installTailscale,
    recoverService,
  }
})
