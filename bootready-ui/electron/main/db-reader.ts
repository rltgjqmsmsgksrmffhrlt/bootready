// db-reader.ts — SQLite 읽기 전용 접근 (Electron Main Process)

import path from 'path'
import os from 'os'
import type Database from 'better-sqlite3'

export interface BootSession {
  id: number
  started_at: string
  completed_at: string | null
  total_duration_ms: number | null
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

export class DbReader {
  private db: Database.Database | null = null

  async init(): Promise<void> {
    const dbPath = this.getDbPath()

    // better-sqlite3는 동기 API — require로 로드
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    const BetterSqlite3 = require('better-sqlite3') as typeof import('better-sqlite3')
    this.db = new BetterSqlite3(dbPath, { readonly: true, fileMustExist: false })
  }

  getLatestSession(): SessionWithEvents | null {
    if (!this.db) return null

    try {
      const session = this.db
        .prepare(
          `SELECT id, started_at, completed_at, total_duration_ms, score
           FROM boot_session ORDER BY id DESC LIMIT 1`
        )
        .get() as BootSession | undefined

      if (!session) return null

      const events = this.db
        .prepare(
          `SELECT id, session_id, name, exe_path, start_ms, end_ms, status
           FROM program_event WHERE session_id = ? ORDER BY start_ms ASC`
        )
        .all(session.id) as ProgramEvent[]

      return { session, events }
    } catch (e) {
      console.error('db read error:', e)
      return null
    }
  }

  getRecentSessions(limit = 10): BootSession[] {
    if (!this.db) return []

    try {
      return this.db
        .prepare(
          `SELECT id, started_at, completed_at, total_duration_ms, score
           FROM boot_session ORDER BY id DESC LIMIT ?`
        )
        .all(limit) as BootSession[]
    } catch {
      return []
    }
  }

  private getDbPath(): string {
    const appdata = process.env.APPDATA ?? path.join(os.homedir(), 'AppData', 'Roaming')
    return path.join(appdata, 'BootReady', 'data.db')
  }
}
