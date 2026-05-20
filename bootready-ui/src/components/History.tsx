import { useEffect, useState } from 'react'
import type { BootSession, SessionWithEvents } from '../types'
import styles from './History.module.css'

interface Props {
  onBack: () => void
  onSelectSession: (data: SessionWithEvents) => void
}

export default function History({ onBack, onSelectSession }: Props) {
  const [sessions, setSessions] = useState<BootSession[]>([])
  const [loading, setLoading] = useState(true)
  const [selectedId, setSelectedId] = useState<number | null>(null)

  useEffect(() => {
    window.api?.getRecentSessions?.(20).then(s => {
      setSessions(s ?? [])
      setLoading(false)
    })
  }, [])

  async function handleSelect(session: BootSession) {
    setSelectedId(session.id)
    const data = await window.api?.getSessionById?.(session.id)
    if (data) onSelectSession(data)
    setSelectedId(null)
  }

  return (
    <div className={styles.root}>
      <div className={styles.header}>
        <button className={styles.backBtn} onClick={onBack}>← 돌아가기</button>
        <span className={styles.headerTitle}>부팅 기록</span>
      </div>

      {loading ? (
        <div className={styles.empty}>불러오는 중…</div>
      ) : sessions.length === 0 ? (
        <div className={styles.empty}>기록이 없습니다.</div>
      ) : (
        <div className={styles.list}>
          {sessions.map(s => (
            <button
              key={s.id}
              className={`${styles.row} ${selectedId === s.id ? styles.rowLoading : ''}`}
              onClick={() => handleSelect(s)}
            >
              <div className={styles.rowLeft}>
                <span className={styles.rowDate}>{fmtDate(s.started_at)}</span>
                <span className={styles.rowTime}>{fmtTime(s.started_at)}</span>
              </div>
              <div className={styles.rowRight}>
                {s.total_duration_ms != null && (
                  <span className={styles.rowDur}>{fmtMs(s.total_duration_ms)}</span>
                )}
                {s.score != null && (
                  <span className={styles.rowScore} style={{ color: scoreColor(s.score) }}>
                    {s.score}점
                  </span>
                )}
              </div>
            </button>
          ))}
        </div>
      )}
    </div>
  )
}

function fmtDate(iso: string) {
  try {
    return new Date(iso).toLocaleDateString('ko-KR', { month: 'short', day: 'numeric', weekday: 'short' })
  } catch { return '—' }
}

function fmtTime(iso: string) {
  try {
    return new Date(iso).toLocaleTimeString('ko-KR', { hour: '2-digit', minute: '2-digit' })
  } catch { return '—' }
}

function fmtMs(ms: number) {
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`
  return `${Math.floor(ms / 60_000)}m ${Math.round((ms % 60_000) / 1000)}s`
}

function scoreColor(s: number) {
  return s >= 80 ? '#1d9e75' : s >= 50 ? '#e09a30' : '#e05050'
}
