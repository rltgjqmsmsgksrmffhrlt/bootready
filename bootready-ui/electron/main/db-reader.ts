// db-reader.ts — SQLite 읽기 전용 접근 (sql.js WASM, 네이티브 컴파일 불필요)

import path from 'path'
import os from 'os'
import fs from 'fs'
import type { Database } from 'sql.js'

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
  private db: Database | null = null

  async init(): Promise<void> {
    const dbPath = this.getDbPath()
    if (!fs.existsSync(dbPath)) return

    // require로 로드해 vite 번들링을 우회 (Electron main = CommonJS 런타임)
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    const initSqlJs = require('sql.js') as typeof import('sql.js')
    const SQL = await initSqlJs()
    const fileBuffer = fs.readFileSync(dbPath)
    this.db = new SQL.Database(fileBuffer)
  }

  getLatestSession(): SessionWithEvents | null {
    if (!this.db) return null
    try {
      const sessionResult = this.db.exec(
        `SELECT id, started_at, completed_at, total_duration_ms, score
         FROM boot_session ORDER BY id DESC LIMIT 1`
      )
      if (!sessionResult.length || !sessionResult[0].values.length) return null

      const session = toObject<BootSession>(sessionResult[0])

      const stmt = this.db.prepare(
        `SELECT id, session_id, name, exe_path, start_ms, end_ms, status
         FROM program_event WHERE session_id = ? ORDER BY start_ms ASC`
      )
      stmt.bind([session.id])
      const events: ProgramEvent[] = []
      while (stmt.step()) events.push(stmt.getAsObject() as unknown as ProgramEvent)
      stmt.free()

      return { session, events }
    } catch (e) {
      console.error('db read error:', e)
      return null
    }
  }

  getRecentSessions(limit = 10): BootSession[] {
    if (!this.db) return []
    try {
      const stmt = this.db.prepare(
        `SELECT id, started_at, completed_at, total_duration_ms, score
         FROM boot_session ORDER BY id DESC LIMIT ?`
      )
      stmt.bind([limit])
      const rows: BootSession[] = []
      while (stmt.step()) rows.push(stmt.getAsObject() as unknown as BootSession)
      stmt.free()
      return rows
    } catch {
      return []
    }
  }

  private getDbPath(): string {
    const appdata = process.env.APPDATA ?? path.join(os.homedir(), 'AppData', 'Roaming')
    return path.join(appdata, 'BootReady', 'data.db')
  }
}

function toObject<T>(result: { columns: string[]; values: SqlValue[][] }): T {
  const obj: Record<string, SqlValue> = {}
  result.columns.forEach((col, i) => { obj[col] = result.values[0][i] })
  return obj as T
}

type SqlValue = string | number | null | Uint8Array
