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
  getFileIcon: (exePath: string) => ipcRenderer.invoke('get-file-icon', exePath),
  setWindowHeight: (h: number) => ipcRenderer.invoke('set-window-height', h),
})
