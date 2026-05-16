import { app, BrowserWindow, ipcMain, screen } from 'electron'
import path from 'path'
import { IpcClient } from './ipc-client'
import { DbReader } from './db-reader'

// Vite 빌드 시 dist로 이동
const DIST = path.join(__dirname, '../../dist')
const DIST_ELECTRON = path.join(__dirname, '../')

let win: BrowserWindow | null = null
let ipcClient: IpcClient | null = null
let dbReader: DbReader | null = null

async function createWindow() {
  // 화면 오른쪽 하단 (트레이 근처)에 팝업 배치
  const { workAreaSize } = screen.getPrimaryDisplay()

  win = new BrowserWindow({
    width: 420,
    height: 560,
    x: workAreaSize.width - 440,
    y: workAreaSize.height - 580,
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

  // 포커스 잃으면 닫기
  win.on('blur', () => {
    if (!process.env.VITE_DEV_SERVER_URL) {
      win?.close()
    }
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
    win?.close()
  })

  ipcMain.handle('open-timeline', () => {
    win?.webContents.send('navigate', '/timeline')
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
  await createWindow()
})

app.on('window-all-closed', () => {
  ipcClient?.disconnect()
  app.quit()
})

app.on('activate', () => {
  if (!win) createWindow()
})
