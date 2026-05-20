import { app, BrowserWindow, ipcMain, screen, Tray, nativeImage } from 'electron'
import path from 'path'
import fs from 'fs'
import os from 'os'
import crypto from 'crypto'
import { IpcClient } from './ipc-client'
import { DbReader } from './db-reader'

// Vite 빌드 시 dist로 이동
const DIST = path.join(__dirname, '../../dist')
const DIST_ELECTRON = path.join(__dirname, '../')

let win: BrowserWindow | null = null
let tray: Tray | null = null
let ipcClient: IpcClient | null = null
let dbReader: DbReader | null = null

// 메모리 캐시
const iconMemCache = new Map<string, string>()

function getIconCacheDir(): string {
  const appdata = process.env.APPDATA ?? path.join(os.homedir(), 'AppData', 'Roaming')
  return path.join(appdata, 'BootReady', 'icons')
}

function getWindowPos(): { x: number; y: number } {
  const { workAreaSize } = screen.getPrimaryDisplay()
  const WIN_W = 360
  const WIN_H = 480
  const OFFSET = 20
  return {
    x: workAreaSize.width - WIN_W - OFFSET,
    y: workAreaSize.height - WIN_H - OFFSET,
  }
}

async function createWindow() {
  const pos = getWindowPos()

  win = new BrowserWindow({
    width: 360,
    height: 480,
    minHeight: 280,
    maxHeight: 480,
    x: pos.x,
    y: pos.y,
    frame: false,
    resizable: false,
    alwaysOnTop: true,
    skipTaskbar: true,
    transparent: false,
    show: false,
    webPreferences: {
      preload: path.join(DIST_ELECTRON, 'preload/index.js'),
      nodeIntegration: false,
      contextIsolation: true,
      sandbox: false,
    },
  })

  if (process.env.VITE_DEV_SERVER_URL) {
    win.loadURL(process.env.VITE_DEV_SERVER_URL)
    win.webContents.openDevTools({ mode: 'detach' })
  } else {
    win.loadFile(path.join(DIST, 'index.html'))
  }

  win.once('ready-to-show', () => {
    win?.show()
    win?.focus()
  })

  // 포커스 잃으면 숨기기 (close 아님 — tray 클릭으로 다시 표시 가능)
  win.on('blur', () => {
    win?.hide()
  })

  win.on('closed', () => {
    win = null
  })
}

// IPC 핸들러 등록
function registerIpcHandlers() {
  // boot-core Named Pipe 또는 SQLite에서 최신 세션 데이터 가져오기
  ipcMain.handle('get-latest-session', async () => {
    // 1. Named Pipe로 boot-core에서 실시간 데이터 시도
    if (ipcClient) {
      try {
        const data = await ipcClient.request('latest_session')
        if (data) return data
      } catch {
        // 파이프 실패 시 DB 폴백
      }
    }

    // 2. SQLite 직접 읽기
    if (dbReader) {
      return dbReader.getLatestSession()
    }

    return null
  })

  ipcMain.handle('get-boot-status', async () => {
    if (ipcClient) {
      try {
        return await ipcClient.request('status')
      } catch {
        // 연결 안 됨 — 정적 상태 반환
      }
    }
    return { is_complete: true, total_programs: 0, active_programs: 0 }
  })

  ipcMain.handle('get-recent-sessions', async (_event, limit: number = 10) => {
    if (dbReader) {
      return dbReader.getRecentSessions(limit)
    }
    return []
  })

  ipcMain.handle('close-window', () => {
    win?.hide()
  })

  ipcMain.handle('open-timeline', () => {
    win?.webContents.send('navigate', '/timeline')
  })

  ipcMain.handle('set-window-height', (_e, h: number) => {
    if (!win) return
    const clamped = Math.max(280, Math.min(480, h))
    win.setContentSize(360, clamped)
  })

  ipcMain.handle('get-file-icon', async (_e, exePath: string) => {
    if (!exePath) return null

    // 메모리 캐시 확인
    if (iconMemCache.has(exePath)) {
      return iconMemCache.get(exePath) ?? null
    }

    // 디스크 캐시 확인
    const hash = crypto.createHash('md5').update(exePath).digest('hex')
    const cacheDir = getIconCacheDir()
    const cachePath = path.join(cacheDir, `${hash}.png`)

    if (fs.existsSync(cachePath)) {
      try {
        const buf = fs.readFileSync(cachePath)
        const dataUrl = `data:image/png;base64,${buf.toString('base64')}`
        iconMemCache.set(exePath, dataUrl)
        return dataUrl
      } catch {
        // 캐시 읽기 실패 시 새로 추출
      }
    }

    // app.getFileIcon 호출
    try {
      const icon = await app.getFileIcon(exePath, { size: 'normal' })
      if (icon.isEmpty()) return null

      const pngBuf = icon.toPNG()
      const dataUrl = `data:image/png;base64,${pngBuf.toString('base64')}`

      // 디스크 캐시 저장
      try {
        fs.mkdirSync(cacheDir, { recursive: true })
        fs.writeFileSync(cachePath, pngBuf)
      } catch {
        // 캐시 저장 실패는 무시
      }

      iconMemCache.set(exePath, dataUrl)
      return dataUrl
    } catch {
      return null
    }
  })
}

function showWindow() {
  if (!win) { createWindow(); return }
  const pos = getWindowPos()
  win.setPosition(pos.x, pos.y)
  win.show()
  win.focus()
}

function watchShowSignal() {
  const appdata = process.env.APPDATA ?? path.join(os.homedir(), 'AppData', 'Roaming')
  const signalPath = path.join(appdata, 'BootReady', 'show.signal')
  setInterval(() => {
    if (fs.existsSync(signalPath)) {
      try { fs.unlinkSync(signalPath) } catch { return }
      showWindow()
    }
  }, 200)
}

function createTray() {
  // 16×16 투명 아이콘 (boot-core가 별도 트레이 아이콘을 그리므로 여기선 최소 크기)
  const icon = nativeImage.createEmpty()
  tray = new Tray(icon)
  tray.setToolTip('BootReady')

  tray.on('click', () => {
    if (win?.isVisible()) { win.hide() } else { showWindow() }
  })
}

app.whenReady().then(async () => {
  // DB 리더 초기화
  try {
    dbReader = new DbReader()
    await dbReader.init()
  } catch (e) {
    console.error('db reader init failed:', e)
  }

  // Named Pipe 클라이언트 연결 시도
  try {
    ipcClient = new IpcClient('\\\\.\\pipe\\bootready')
    await ipcClient.connect()
  } catch {
    console.log('boot-core pipe not available (using DB only)')
    ipcClient = null
  }

  registerIpcHandlers()
  createTray()
  watchShowSignal()
  await createWindow()
})

// hide 방식이라 window-all-closed는 발생하지 않음 — tray 우클릭 메뉴로 종료
app.on('window-all-closed', () => {
  // macOS 외엔 tray가 살아있으므로 quit 하지 않음
  if (process.platform !== 'darwin') return
  ipcClient?.disconnect()
  app.quit()
})

app.on('activate', () => {
  if (!win) createWindow()
})
