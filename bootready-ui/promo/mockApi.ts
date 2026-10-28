import type { SessionWithEvents } from '../src/types'

// 실제 앱의 config.json 자리. 설정 화면(iframe)과 부팅 시뮬레이션(캐러셀)이 같은 값을 읽는다.
export const CONFIG_KEY = 'bootready-promo-config'

// 실제로 등록해 쓰는 주소 두 개(공유 추적 꼬리표 utm_source는 제거).
const DEFAULT_CONFIG = {
  idleSeconds: 5,
  slowThresholdMs: 30000,
  startup_urls: [
    'https://chromewebstore.google.com/detail/notion-switcher/inbafaehomballlkcenijimmipjfikem',
    'https://loquacious-biscuit-ceac87.netlify.app/',
  ],
}

export function readConfig(): Record<string, unknown> {
  try {
    const raw = localStorage.getItem(CONFIG_KEY)
    if (raw) return JSON.parse(raw)
  } catch {}
  return { ...DEFAULT_CONFIG }
}

export function readUrls(): string[] {
  const urls = readConfig().startup_urls
  return Array.isArray(urls) ? (urls as string[]) : []
}

function writeConfig(config: Record<string, unknown>) {
  try {
    localStorage.setItem(CONFIG_KEY, JSON.stringify(config))
  } catch {}
}

// 가짜 값. 실제 측정값이 아니다. 브랜드 이름은 쓰지 않는다.
function buildSession(): SessionWithEvents {
  const started = new Date()
  started.setHours(9, 0, 0, 0)
  const ok = 'ok' as const
  const slow = 'slow' as const
  return {
    session: {
      id: 1,
      started_at: started.toISOString(),
      completed_at: new Date(started.getTime() + 47_200).toISOString(),
      total_duration_ms: 47_200,
      settled_duration_ms: 23_000,
      score: 82,
    },
    events: [
      { id: 1, session_id: 1, name: 'ChatApp', exe_path: 'C:\\Program Files\\ChatApp\\chat.exe', start_ms: 800, end_ms: 5_200, status: ok },
      { id: 2, session_id: 1, name: 'CloudSync', exe_path: 'C:\\Program Files\\CloudSync\\sync.exe', start_ms: 1_100, end_ms: 9_800, status: ok },
      { id: 3, session_id: 1, name: 'GameLauncher', exe_path: 'C:\\Program Files\\GameLauncher\\launcher.exe', start_ms: 2_400, end_ms: 38_500, status: slow },
      { id: 4, session_id: 1, name: 'NoteApp', exe_path: 'C:\\Program Files\\NoteApp\\note.exe', start_ms: 4_200, end_ms: 7_600, status: ok },
    ],
  }
}

// 브라우저에서 실제 App을 돌리기 위한 window.api 대역.
export function installMockApi() {
  const session = buildSession()
  const initialRoute = new URLSearchParams(location.search).get('route')

  window.api = {
    getLatestSession: () => Promise.resolve(session),
    getBootStatus: () =>
      Promise.resolve({ is_complete: true, total_programs: 4, active_programs: 4, total_ms: 47_200, settled_ms: 23_000, score: 82 }),
    getRecentSessions: () => Promise.resolve([session.session]),
    getSessionById: () => Promise.resolve(session),
    closeWindow: () => { window.parent.postMessage({ type: 'close' }, '*') },
    openTimeline: () => {},
    onNavigate: (cb) => {
      if (initialRoute === 'settings') cb('/settings')
    },
    onSessionUpdated: () => {},
    onUpdateAvailable: () => {},
    getAutostart: () => Promise.resolve(true),
    setAutostart: () => Promise.resolve(true),
    saveConfig: (config) => { writeConfig(config); return Promise.resolve(true) },
    loadConfig: () => Promise.resolve(readConfig()),
    setWindowHeight: (h) => { window.parent.postMessage({ type: 'height', h }, '*'); return Promise.resolve() },
    quitApp: () => Promise.resolve(),
    openReleasePage: () => Promise.resolve(),
  }
}
