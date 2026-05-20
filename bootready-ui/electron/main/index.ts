import { app, BrowserWindow, ipcMain, screen, Tray, nativeImage, Menu } from 'electron'
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
    if (dbReader) return dbReader.getRecentSessions(limit)
    return []
  })

  ipcMain.handle('get-session-by-id', async (_event, id: number) => {
    if (dbReader) return dbReader.getSessionById(id)
    return null
  })

  ipcMain.handle('close-window', () => {
    win?.hide()
  })

  ipcMain.handle('open-timeline', () => {
    win?.webContents.send('navigate', '/timeline')
  })

  ipcMain.handle('get-autostart', () => {
    try {
      const { execSync } = require('child_process')
      const out = execSync(
        'reg query "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run" /v BootReady',
        { stdio: ['ignore', 'pipe', 'ignore'] }
      ).toString()
      return out.includes('BootReady')
    } catch { return false }
  })

  ipcMain.handle('set-autostart', (_e, enable: boolean) => {
    const appdata = process.env.APPDATA ?? path.join(os.homedir(), 'AppData', 'Roaming')
    const exePath = path.join(appdata, 'BootReady', 'boot-core.exe')
    try {
      const { execSync } = require('child_process')
      if (enable) {
        execSync(
          `reg add "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run" /v BootReady /t REG_SZ /d "${exePath}" /f`,
          { stdio: 'ignore' }
        )
      } else {
        execSync(
          'reg delete "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run" /v BootReady /f',
          { stdio: 'ignore' }
        )
      }
      return true
    } catch { return false }
  })

  ipcMain.handle('save-config', (_e, config: Record<string, unknown>) => {
    const appdata = process.env.APPDATA ?? path.join(os.homedir(), 'AppData', 'Roaming')
    const configPath = path.join(appdata, 'BootReady', 'config.json')
    try {
      fs.mkdirSync(path.dirname(configPath), { recursive: true })
      fs.writeFileSync(configPath, JSON.stringify(config, null, 2))
      return true
    } catch { return false }
  })

  ipcMain.handle('load-config', () => {
    const appdata = process.env.APPDATA ?? path.join(os.homedir(), 'AppData', 'Roaming')
    const configPath = path.join(appdata, 'BootReady', 'config.json')
    try {
      return JSON.parse(fs.readFileSync(configPath, 'utf8'))
    } catch { return null }
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
  const dir = path.join(appdata, 'BootReady')
  const signalPath = path.join(dir, 'show.signal')

  fs.mkdirSync(dir, { recursive: true })

  // fs.watch로 디렉토리 감시 — 파일 생성 이벤트만 처리
  try {
    fs.watch(dir, (event, filename) => {
      if (filename !== 'show.signal') return
      if (!fs.existsSync(signalPath)) return
      try { fs.unlinkSync(signalPath) } catch { return }
      showWindow()
    })
  } catch {
    // fs.watch 실패 시 폴링 fallback
    setInterval(() => {
      if (fs.existsSync(signalPath)) {
        try { fs.unlinkSync(signalPath) } catch { return }
        showWindow()
      }
    }, 500)
  }
}

function createTray() {
  const icon = nativeImage.createEmpty()
  tray = new Tray(icon)
  tray.setToolTip('BootReady')

  tray.on('click', () => {
    if (win?.isVisible()) { win.hide() } else { showWindow() }
  })

  const menu = Menu.buildFromTemplate([
    {
      label: 'BootReady 열기',
      click: () => showWindow(),
    },
    { type: 'separator' },
    {
      label: '종료',
      click: () => quitApp(),
    },
  ])
  tray.setContextMenu(menu)
}

// boot-core 파이프 재연결 + 세션 완료 감지 → 창 자동 갱신
function startPipeWatcher() {
  let wasComplete = false

  setInterval(async () => {
    // 연결 끊겼으면 재연결 시도
    if (!ipcClient?.isConnected) {
      try {
        ipcClient = new IpcClient('\\\\.\\pipe\\bootready')
        await ipcClient.connect()
        console.log('pipe reconnected')
      } catch {
        ipcClient = null
        return
      }
    }

    // 세션 완료 감지 → 창이 열려 있으면 갱신 신호 전송
    try {
      const status = await ipcClient!.request('status') as { is_complete?: boolean } | null
      const isComplete = status?.is_complete ?? false
      if (isComplete && !wasComplete) {
        wasComplete = true
        win?.webContents.send('session-updated')
      }
      if (!isComplete) wasComplete = false
    } catch { /* ignore */ }
  }, 3000)
}

function quitApp() {
  ipcClient?.disconnect()
  tray?.destroy()
  app.quit()
}

function setupBootCore() {
  // 개발 환경에서는 스킵
  if (!app.isPackaged) return

  const appdata = process.env.APPDATA ?? path.join(os.homedir(), 'AppData', 'Roaming')
  const installDir = path.join(appdata, 'BootReady')
  const destExe = path.join(installDir, 'boot-core.exe')

  // extraResources에서 복사
  const srcExe = path.join(process.resourcesPath, 'boot-core.exe')
  if (fs.existsSync(srcExe)) {
    try {
      fs.mkdirSync(installDir, { recursive: true })
      fs.copyFileSync(srcExe, destExe)
    } catch (e) {
      console.error('boot-core copy failed:', e)
      return
    }
  }

  if (!fs.existsSync(destExe)) return

  // 레지스트리 Run 키에 시작프로그램 등록
  try {
    const { execSync } = require('child_process')
    execSync(
      `reg add "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run" /v BootReady /t REG_SZ /d "${destExe}" /f`,
      { stdio: 'ignore' }
    )
    console.log('boot-core registered as startup program')
  } catch (e) {
    console.error('startup registration failed:', e)
  }

  // boot-core가 실행 중이 아니면 즉시 시작
  try {
    const { spawn } = require('child_process')
    spawn(destExe, [], { detached: true, stdio: 'ignore' }).unref()
  } catch (e) {
    console.error('boot-core launch failed:', e)
  }
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

  setupBootCore()
  registerIpcHandlers()
  createTray()
  watchShowSignal()
  startPipeWatcher()
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
