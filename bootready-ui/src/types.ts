export interface BootSession {
  id: number
  started_at: string
  completed_at: string | null
  total_duration_ms: number | null
  settled_duration_ms: number | null
  score: number | null
}

export interface ProgramEvent {
  id: number
  session_id: number
  name: string
  exe_path: string | null
  start_ms: number | null
  end_ms: number | null
  status: 'ok' | 'slow' | 'disabled' | 'failed'
}

export interface SessionWithEvents {
  session: BootSession
  events: ProgramEvent[]
}

export interface BootStatus {
  is_complete: boolean
  total_programs: number
  active_programs: number
  total_ms: number | null
  settled_ms: number | null
  score: number | null
}

// Tauri API 타입
declare global {
  interface Window {
    api: {
      getLatestSession: () => Promise<SessionWithEvents | null>
      getBootStatus: () => Promise<BootStatus>
      getRecentSessions: (limit?: number) => Promise<BootSession[]>
      closeWindow: () => void
      openTimeline: () => void
      onNavigate: (cb: (route: string) => void) => void
      onSessionUpdated: (cb: () => void) => void
      onUpdateAvailable: (cb: (version: string) => void) => void
      getSessionById: (id: number) => Promise<SessionWithEvents | null>
      getAutostart: () => Promise<boolean>
      setAutostart: (enable: boolean) => Promise<boolean>
      saveConfig: (config: Record<string, unknown>) => Promise<boolean>
      loadConfig: () => Promise<Record<string, unknown> | null>
      setWindowHeight: (h: number) => Promise<void>
      quitApp: () => Promise<void>
      openReleasePage: () => Promise<void>
    }
  }
}
