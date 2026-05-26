import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import './styles/globals.css'

const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

async function setup() {
  if (isTauri) {
    const { invoke } = await import('@tauri-apps/api/core')
    const { listen } = await import('@tauri-apps/api/event')

    ;(window as Window).api = {
      getLatestSession: () => invoke('get_latest_session'),
      getBootStatus: () => invoke('get_boot_status'),
      getRecentSessions: (limit) => invoke('get_recent_sessions', { limit }),
      getSessionById: (id) => invoke('get_session_by_id', { id }),
      closeWindow: () => invoke('close_window'),
      openTimeline: () => invoke('open_timeline'),
      onNavigate: (cb) => { listen<string>('navigate', e => cb(e.payload)) },
      onSessionUpdated: (cb) => { listen('session-updated', () => cb()) },
      onUpdateAvailable: (cb) => { listen<string>('update-available', e => cb(e.payload)) },
      getAutostart: () => invoke('get_autostart'),
      setAutostart: (enable) => invoke('set_autostart', { enable }),
      saveConfig: (config) => invoke('save_config', { config }),
      loadConfig: () => invoke('load_config'),
      setWindowHeight: (h) => invoke('set_window_height', { h }),
      quitApp: () => invoke('quit_app'),
      openReleasePage: () => invoke('open_release_page'),
    }
  } else {
    // 브라우저 개발 환경 mock
    const mockSession = {
      session: {
        id: 1,
        started_at: new Date(Date.now() - 47_200).toISOString(),
        completed_at: new Date().toISOString(),
        total_duration_ms: 47_200,
        score: 82,
      },
      events: [
        { id: 1, session_id: 1, name: 'Discord', exe_path: 'C:\\Users\\User\\AppData\\Local\\Discord\\Update.exe', start_ms: 800, end_ms: 5_200, status: 'ok' as const },
        { id: 2, session_id: 1, name: 'Slack', exe_path: 'C:\\Users\\User\\AppData\\Local\\slack\\slack.exe', start_ms: 1_100, end_ms: 9_800, status: 'slow' as const },
        { id: 3, session_id: 1, name: 'Steam', exe_path: 'C:\\Program Files (x86)\\Steam\\steam.exe', start_ms: 2_400, end_ms: 18_500, status: 'slow' as const },
        { id: 4, session_id: 1, name: 'Microsoft OneDrive', exe_path: 'C:\\Program Files\\Microsoft OneDrive\\OneDrive.exe', start_ms: 3_000, end_ms: 12_100, status: 'ok' as const },
        { id: 5, session_id: 1, name: 'Notion', exe_path: 'C:\\Users\\User\\AppData\\Local\\Programs\\Notion\\Notion.exe', start_ms: 4_200, end_ms: 7_600, status: 'ok' as const },
      ],
    }

    ;(window as Window).api = {
      getLatestSession: () => Promise.resolve(mockSession),
      getBootStatus: () => Promise.resolve({ is_complete: true, total_programs: 5, active_programs: 5, total_ms: 47200, score: 82 }),
      getRecentSessions: () => Promise.resolve([mockSession.session]),
      getSessionById: () => Promise.resolve(mockSession),
      closeWindow: () => {},
      openTimeline: () => {},
      onNavigate: () => {},
      onSessionUpdated: () => {},
      onUpdateAvailable: () => {},
      getAutostart: () => Promise.resolve(true),
      setAutostart: () => Promise.resolve(true),
      saveConfig: () => Promise.resolve(true),
      loadConfig: () => Promise.resolve(null),
      setWindowHeight: () => Promise.resolve(),
      quitApp: () => Promise.resolve(),
      openReleasePage: () => Promise.resolve(),
    }
  }

  ReactDOM.createRoot(document.getElementById('root')!).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>
  )
}

setup()
