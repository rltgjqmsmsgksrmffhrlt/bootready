import { useMemo } from 'react'
import type { SessionWithEvents, ProgramEvent } from '../types'
import styles from './Timeline.module.css'

interface Props {
  sessionData: SessionWithEvents | null
  onBack: () => void
}

export default function Timeline({ sessionData, onBack }: Props) {
  const events = sessionData?.events ?? []
  const totalMs = useMemo(() => {
    if (sessionData?.session?.total_duration_ms) return sessionData.session.total_duration_ms
    const maxEnd = Math.max(0, ...events.map(e => e.end_ms ?? e.start_ms ?? 0))
    return maxEnd > 0 ? maxEnd * 1.1 : 30_000
  }, [sessionData, events])

  // 간트차트용 정렬 및 정규화
  const rows = useMemo(() => {
    return [...events]
      .filter((e) => e.start_ms != null)
      .sort((a, b) => (a.start_ms ?? 0) - (b.start_ms ?? 0))
      .map((e) => {
        const startPct = ((e.start_ms ?? 0) / totalMs) * 100
        const endMs = e.end_ms ?? totalMs
        const widthPct = Math.max(((endMs - (e.start_ms ?? 0)) / totalMs) * 100, 0.5)
        return { ...e, startPct, widthPct }
      })
  }, [events, totalMs])

  // 축 눈금 (5개)
  const ticks = useMemo(() => {
    const count = 5
    return Array.from({ length: count + 1 }, (_, i) => ({
      pct: (i / count) * 100,
      label: formatDuration((i / count) * totalMs),
    }))
  }, [totalMs])

  return (
    <div className={styles.root}>
      {/* 헤더 */}
      <div className={styles.header}>
        <button className={styles.backBtn} onClick={onBack}>
          ← 돌아가기
        </button>
        <span className={styles.headerTitle}>부팅 타임라인</span>
        <span className={styles.headerSub}>
          {sessionData?.session
            ? `부팅 ${formatDuration(totalMs)}${sessionData.session.settled_duration_ms != null ? ` · 안정화 ${formatDuration(sessionData.session.settled_duration_ms)}` : ''} · ${events.length}개 프로그램`
            : '데이터 없음'}
        </span>
      </div>

      {rows.length === 0 ? (
        <div className={styles.empty}>타임라인 데이터가 없습니다.</div>
      ) : (
        <div className={styles.chart}>
          {/* 시간축 */}
          <div className={styles.axis}>
            {ticks.map((t) => (
              <div
                key={t.pct}
                className={styles.tick}
                style={{ left: `${t.pct}%` }}
              >
                <div className={styles.tickLine} />
                <span className={styles.tickLabel}>{t.label}</span>
              </div>
            ))}
          </div>

          {/* 간트 행 */}
          <div className={styles.rows}>
            {rows.map((row) => (
              <GanttRow key={row.id} row={row} />
            ))}
          </div>
        </div>
      )}
    </div>
  )
}

type GanttRow = ProgramEvent & { startPct: number; widthPct: number }

function GanttRow({ row }: { row: GanttRow }) {
  const duration =
    row.start_ms != null && row.end_ms != null
      ? row.end_ms - row.start_ms
      : null

  const barColor: Record<string, string> = {
    ok: '#1d9e75',
    slow: '#e09a30',
    failed: '#e05050',
    disabled: '#4a4844',
  }

  return (
    <div className={styles.ganttRow}>
      <div className={styles.rowLabel} title={row.name}>
        {row.name}
      </div>
      <div className={styles.rowBar}>
        {/* 그리드 세로선 */}
        {[20, 40, 60, 80].map((p) => (
          <div key={p} className={styles.gridLine} style={{ left: `${p}%` }} />
        ))}
        {/* 바 */}
        <div
          className={styles.bar}
          style={{
            left: `${row.startPct}%`,
            width: `${row.widthPct}%`,
            background: barColor[row.status] ?? barColor.ok,
          }}
          title={`${row.name}: ${duration != null ? formatDuration(duration) : '?'}`}
        />
      </div>
      <div className={styles.rowDuration}>
        {duration != null ? formatDuration(duration) : '—'}
      </div>
    </div>
  )
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)}ms`
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`
  return `${Math.floor(ms / 60_000)}m${Math.round((ms % 60_000) / 1000)}s`
}
