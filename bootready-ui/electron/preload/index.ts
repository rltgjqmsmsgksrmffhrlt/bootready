import { contextBridge, ipcRenderer } from 'electron'

// Renderer에서 window.api.xxx 로 접근
contextBridge.exposeInMainWorld('api', {
  getLatestSession: () => ipcRenderer.invoke('get-latest-session'),
  getBootStatus: () => ipcRenderer.invoke('get-boot-status'),
  getRecentSessions: (limit?: number) => ipcRenderer.invoke('get-recent-sessions', limit),
  closeWindow: () => ipcRenderer.invoke('close-window'),
  openTimeline: () => ipcRenderer.invoke('open-timeline'),
  onNavigate: (cb: (route: string) => void) =>
    ipcRenderer.on('navigate', (_e, route) => cb(route)),
  onSessionUpdated: (cb: () => void) =>
    ipcRenderer.on('session-updated', () => cb()),
  getFileIcon: (exePath: string) => ipcRenderer.invoke('get-file-icon', exePath),
  setWindowHeight: (h: number) => ipcRenderer.invoke('set-window-height', h),
  getSessionById: (id: number) => ipcRenderer.invoke('get-session-by-id', id),
  getAutostart: () => ipcRenderer.invoke('get-autostart'),
  setAutostart: (enable: boolean) => ipcRenderer.invoke('set-autostart', enable),
  saveConfig: (config: Record<string, unknown>) => ipcRenderer.invoke('save-config', config),
  loadConfig: () => ipcRenderer.invoke('load-config'),
})
