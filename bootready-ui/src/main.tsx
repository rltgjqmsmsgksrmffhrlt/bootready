import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import './styles/globals.css'

// 브라우저 개발 환경: Electron IPC 없이 mock 데이터로 렌더링
if (!window.api) {
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
    closeWindow: () => {},
    openTimeline: () => {},
    onNavigate: () => {},
  }
}

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
)
