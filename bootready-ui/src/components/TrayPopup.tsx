import type { SessionWithEvents, ProgramEvent } from '../types'
import styles from './TrayPopup.module.css'

interface Props {
  sessionData: SessionWithEvents | null
  onOpenTimeline: () => void
  onOpenSettings: () => void
  onClose: () => void
}

export default function TrayPopup({ sessionData, onOpenTimeline, onOpenSettings, onClose }: Props) {
  const session = sessionData?.session
  const events = sessionData?.events ?? []

  const totalMs = session?.total_duration_ms
  const score = session?.score

  return (
    <div className={styles.root}>
      {/* 타이틀 바 */}
      <div className={styles.titleBar}>
        <div className={styles.titleLeft}>
          <span className={styles.logo}>●</span>
          <span className={styles.title}>BootReady</span>
        </div>
        <div className={styles.titleActions}>
          <button className={styles.iconBtn} onClick={onOpenSettings} title="설정">
            ⚙
          </button>
          <button className={styles.iconBtn} onClick={onClose} title="닫기">
            ✕
          </button>
        </div>
      </div>

      {/* 부팅 요약 */}
      <div className={styles.summary}>
        {session ? (
          <>
            <div className={styles.summaryRow}>
              <div className={styles.summaryItem}>
                <span className={styles.summaryLabel}>총 부팅 시간</span>
                <span className={styles.summaryValue}>
                  {totalMs != null ? formatDuration(totalMs) : '—'}
                </span>
              </div>
              <div className={styles.summaryItem}>
                <span className={styles.summaryLabel}>시작프로그램</span>
                <span className={styles.summaryValue}>{events.length}개</span>
              </div>
              <div className={styles.summaryItem}>
                <span className={styles.summaryLabel}>부팅 시각</span>
                <span className={styles.summaryValue}>
                  {formatTime(session.started_at)}
                </span>
              </div>
            </div>

            {/* 부팅 점수 */}
            {score != null && (
              <div className={styles.scoreBar}>
                <div className={styles.scoreLabel}>
                  <span>부팅 점수</span>
                  <span className={styles.scoreNum} style={{ color: scoreColor(score) }}>
                    {score}점
                  </span>
                </div>
                <div className={styles.scoreTrack}>
                  <div
                    className={styles.scoreFill}
                    style={{
                      width: `${score}%`,
                      background: scoreColor(score),
                    }}
                  />
                </div>
              </div>
            )}
          </>
        ) : (
          <div className={styles.noData}>
            부팅 데이터가 없습니다.
            <br />
            <span className={styles.noDataHint}>재부팅 후 데이터가 수집됩니다.</span>
          </div>
        )}
      </div>

      {/* 프로그램 목록 */}
      {events.length > 0 && (
        <div className={styles.list}>
          <div className={styles.listHeader}>시작프로그램 목록</div>
          <div className={styles.listBody}>
            {events.map((ev) => (
              <ProgramRow key={ev.id} event={ev} />
            ))}
          </div>
        </div>
      )}

      {/* 하단 버튼 */}
      <div className={styles.footer}>
        <button className={styles.timelineBtn} onClick={onOpenTimeline} disabled={events.length === 0}>
          타임라인 보기
        </button>
      </div>
    </div>
  )
}

function ProgramRow({ event }: { event: ProgramEvent }) {
  const duration =
    event.start_ms != null && event.end_ms != null
      ? event.end_ms - event.start_ms
      : null

  return (
    <div className={styles.programRow}>
      <StatusDot status={event.status} />
      <div className={styles.programInfo}>
        <span className={styles.programName}>{event.name}</span>
        {event.exe_path && (
          <span className={styles.programPath} title={event.exe_path}>
            {shortPath(event.exe_path)}
          </span>
        )}
      </div>
      <div className={styles.programMeta}>
        {duration != null && (
          <span className={styles.programDuration}>{formatDuration(duration)}</span>
        )}
        {event.status === 'slow' && (
          <span className={styles.badge} style={{ background: '#4a3200', color: '#e09a30' }}>
            느림
          </span>
        )}
        {event.status === 'failed' && (
          <span className={styles.badge} style={{ background: '#3a1010', color: '#e05050' }}>
            실패
          </span>
        )}
      </div>
    </div>
  )
}

function StatusDot({ status }: { status: string }) {
  const colors: Record<string, string> = {
    ok: '#1d9e75',
    slow: '#e09a30',
    failed: '#e05050',
    disabled: '#6a6864',
  }
  return (
    <div
      style={{
        width: 8,
        height: 8,
        borderRadius: '50%',
        background: colors[status] ?? '#6a6864',
        flexShrink: 0,
        marginTop: 2,
      }}
    />
  )
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`
  return `${Math.floor(ms / 60_000)}m ${Math.round((ms % 60_000) / 1000)}s`
}

function formatTime(isoStr: string): string {
  try {
    return new Date(isoStr).toLocaleTimeString('ko-KR', {
      hour: '2-digit',
      minute: '2-digit',
    })
  } catch {
    return '—'
  }
}

function shortPath(p: string): string {
  const parts = p.replace(/\\/g, '/').split('/')
  return parts.length > 2 ? `…/${parts.slice(-2).join('/')}` : p
}

function scoreColor(score: number): string {
  if (score >= 80) return '#1d9e75'
  if (score >= 50) return '#e09a30'
  return '#e05050'
}
