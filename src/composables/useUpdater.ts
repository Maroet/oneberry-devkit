import { ref, onMounted, onUnmounted } from 'vue'
import { check, type Update } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'

export type UpdateState = 'idle' | 'checking' | 'available' | 'downloading' | 'installing' | 'error'

// GitLab Deploy Token (只读权限: read_repository + read_package_registry)
// 在 GitLab 项目 Settings → Repository → Deploy tokens 中创建
const GITLAB_DEPLOY_TOKEN = 'gldt-fTWo3LxT-kuau1KVeA6r'

export function useUpdater() {
  const state = ref<UpdateState>('idle')
  const update = ref<Update | null>(null)
  const newVersion = ref('')
  const releaseNotes = ref('')
  const downloadProgress = ref(0)
  const totalSize = ref(0)
  const errorMsg = ref('')
  const dismissed = ref(false)

  let checkTimer: ReturnType<typeof setTimeout> | null = null

  async function checkForUpdates() {
    try {
      state.value = 'checking'
      const result = await check({
        headers: {
          'PRIVATE-TOKEN': GITLAB_DEPLOY_TOKEN,
        },
      })
      if (result) {
        update.value = result
        newVersion.value = result.version
        releaseNotes.value = result.body ?? ''
        state.value = 'available'
        dismissed.value = false
      } else {
        state.value = 'idle'
      }
    } catch (e) {
      console.warn('更新检查失败:', e)
      state.value = 'idle' // 静默失败，不打扰用户
    }
  }

  async function installUpdate() {
    if (!update.value) return
    try {
      state.value = 'downloading'
      downloadProgress.value = 0
      totalSize.value = 0

      await update.value.downloadAndInstall((event) => {
        switch (event.event) {
          case 'Started':
            totalSize.value = event.data.contentLength ?? 0
            break
          case 'Progress':
            downloadProgress.value += event.data.chunkLength
            break
          case 'Finished':
            state.value = 'installing'
            break
        }
      })

      // 安装完成，重启
      await relaunch()
    } catch (e) {
      errorMsg.value = String(e)
      state.value = 'error'
    }
  }

  function dismiss() {
    dismissed.value = true
  }

  onMounted(() => {
    // 启动 3 秒后首次检查
    checkTimer = setTimeout(checkForUpdates, 3000)
  })

  onUnmounted(() => {
    if (checkTimer) clearTimeout(checkTimer)
  })

  return {
    state,
    newVersion,
    releaseNotes,
    downloadProgress,
    totalSize,
    errorMsg,
    dismissed,
    checkForUpdates,
    installUpdate,
    dismiss,
  }
}
