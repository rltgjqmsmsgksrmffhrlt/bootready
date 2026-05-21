import { useState, useEffect, useRef } from 'react'
import type { SessionWithEvents, ProgramEvent } from '../types'
import styles from './TrayPopup.module.css'

interface Props {
  sessionData: SessionWithEvents | null
  onOpenTimeline: () => void
  onOpenSettings: () => void
  onOpenHistory: () => void
  onClose: () => void
}

type Filter = 'all' | 'slow' | 'failed'

export default function TrayPopup({ sessionData, onOpenTimeline, onOpenSettings, onOpenHistory, onClose }: Props) {
  const session = sessionData?.session
  const events = sessionData?.events ?? []
  const [filter, setFilter] = useState<Filter>('all')
  const [hideMicrosoft, setHideMicrosoft] = useState(false)
  const [icons, setIcons] = useState<Record<string, string>>({})
  const [scoreExpanded, setScoreExpanded] = useState(false)
  const rootRef = useRef<HTMLDivElement>(null)

  // 앱 아이콘 로드
  useEffect(() => {
    for (const ev of events) {
      if (!ev.exe_path || icons[ev.exe_path]) continue
      window.api.getFileIcon(ev.exe_path).then(b64 => {
        if (b64) setIcons(prev => ({ ...prev, [ev.exe_path!]: b64 }))
      })
    }
  }, [events])

  // 창 높이 자동 조절
  useEffect(() => {
    if (!rootRef.current) return
    const h = rootRef.current.scrollHeight
    window.api.setWindowHeight(h)
  })

  // session이 미완료면 events에서 추정
  const derivedTotalMs = session?.total_duration_ms
    ?? (events.length > 0 ? Math.max(0, ...events.map(e => e.end_ms ?? 0)) : null)
  const derivedScore = session?.score
    ?? (derivedTotalMs != null ? estimateScore(derivedTotalMs, events.length) : null)
  const isEstimated = session != null && (session.total_duration_ms == null || session.score == null)

  const displayed = events
    .filter(ev => filter === 'all' || ev.status === filter)
    .filter(ev => !hideMicrosoft || !isMicrosoft(ev.exe_path))

  const slowCount = events.filter(e => e.status === 'slow').length
  const failedCount = events.filter(e => e.status === 'failed').length

  return (
    <div className={styles.root} ref={rootRef}>
      {/* 타이틀 바 */}
      <div className={styles.titleBar}>
        <div className={styles.titleLeft}>
          <span className={styles.logo}>●</span>
          <span className={styles.title}>BootReady</span>
        </div>
        <div className={styles.titleActions}>
          <button className={styles.iconBtn} onClick={onOpenSettings} title="설정">⚙</button>
          <button className={`${styles.iconBtn} ${styles.iconBtnClose}`} onClick={onClose} title="닫기">✕</button>
        </div>
      </div>

      {/* 요약 */}
      <div className={styles.summary}>
        {session ? (
          <>
            {/* 점수 — 상단 */}
            {derivedScore != null && (
              <div className={styles.scoreSection}>
                <div className={styles.scoreSectionHeader}>
                  <div className={styles.scoreSectionLeft}>
                    <span className={styles.scoreSectionLabel}>
                      부팅 점수{isEstimated && <span className={styles.estimatedTag}> 측정 중</span>}
                    </span>
                    <span className={styles.scoreSectionNum} style={{ color: scoreColor(derivedScore) }}>
                      {derivedScore}점
                    </span>
                  </div>
                  <button
                    className={styles.reasonBtn}
                    onClick={() => setScoreExpanded(v => !v)}
                  >
                    {scoreExpanded ? '접기' : '이유 보기'}
                  </button>
                </div>
                <div className={styles.scoreTrack}>
                  <div
                    className={styles.scoreFill}
                    style={{ width: `${derivedScore}%`, background: scoreColor(derivedScore) }}
                  />
                </div>
                {scoreExpanded && (
                  <div className={styles.scoreReasonBox}>
                    {buildScoreReasonLines(derivedTotalMs, events).map((line, i) => (
                      <div key={i} className={styles.scoreReasonLine}>{line}</div>
                    ))}
                  </div>
                )}
              </div>
            )}

            {/* 3열 요약 */}
            <div className={styles.summaryRow}>
              <div className={styles.summaryItem}>
                <span className={styles.summaryLabel}>총 부팅 시간</span>
                <span className={styles.summaryValue}>{fmtMs(derivedTotalMs)}</span>
              </div>
              <div className={styles.summaryItem}>
                <span className={styles.summaryLabel}>시작프로그램</span>
                <span className={styles.summaryValue}>{events.length}개</span>
              </div>
              <div className={styles.summaryItem}>
                <span className={styles.summaryLabel}>부팅 시각</span>
                <span className={styles.summaryValue}>{fmtTime(session.started_at)}</span>
              </div>
            </div>
          </>
        ) : (
          <div className={styles.noData}>
            부팅 데이터가 없습니다.
            <br />
            <span className={styles.noDataHint}>재부팅 후 데이터가 수집됩니다.</span>
          </div>
        )}
      </div>

      {/* 필터 */}
      {events.length > 0 && (
        <div className={styles.filterBar}>
          <div className={styles.filterChips}>
            {([
              { f: 'all',    label: `전체 ${events.length}` },
              { f: 'slow',   label: `느림 ${slowCount}` },
              { f: 'failed', label: `실패 ${failedCount}` },
            ] as { f: Filter; label: string }[]).map(({ f, label }) => (
              <button
                key={f}
                className={`${styles.chip} ${filter === f ? styles.chipActive : ''}`}
                onClick={() => setFilter(f)}
              >
                {label}
              </button>
            ))}
          </div>
          <button
            className={`${styles.msToggle} ${hideMicrosoft ? styles.msToggleActive : ''}`}
            onClick={() => setHideMicrosoft(v => !v)}
            title="Microsoft 시스템 프로그램 제외"
          >
            ● MS
          </button>
        </div>
      )}

      {/* 프로그램 목록 */}
      {events.length > 0 && (
        <div className={styles.list}>
          <div className={styles.listBody}>
            {displayed.length === 0 ? (
              <div className={styles.emptyFilter}>해당 항목 없음</div>
            ) : (
              displayed.map(ev => (
                <ProgramRow key={ev.id} event={ev} icon={icons[ev.exe_path ?? ''] ?? null} />
              ))
            )}
          </div>
        </div>
      )}

      {/* 하단 버튼 */}
      <div className={styles.footer}>
        <button className={styles.timelineBtn} onClick={onOpenTimeline} disabled={events.length === 0}>
          타임라인 보기
        </button>
        <button className={styles.historyBtn} onClick={onOpenHistory}>
          기록
        </button>
      </div>
    </div>
  )
}

function ProgramRow({ event, icon }: { event: ProgramEvent; icon: string | null }) {
  const duration =
    event.start_ms != null && event.end_ms != null ? event.end_ms - event.start_ms : null

  return (
    <div className={styles.programRow} title={event.exe_path ?? ''}>
      <div className={styles.appIcon}>
        {icon ? (
          <img src={icon} width={24} height={24} alt="" />
        ) : (
          <div className={styles.appIconFallback} />
        )}
      </div>
      <span className={styles.programName}>{event.name}</span>
      <div className={styles.programMeta}>
        {event.status === 'slow' && (
          <span className={styles.badge} style={{ background: '#4a3200', color: '#e09a30' }}>느림</span>
        )}
        {event.status === 'failed' && (
          <span className={styles.badge} style={{ background: '#3a1010', color: '#e05050' }}>실패</span>
        )}
        {duration != null && (
          <span className={styles.programDuration}>{fmtMs(duration)}</span>
        )}
      </div>
    </div>
  )
}

function isMicrosoft(exePath: string | null): boolean {
  if (!exePath) return false
  const p = exePath.toLowerCase()
  return (
    p.startsWith('c:\\windows\\') ||
    p.includes('program files\\windowsapps\\microsoft.') ||
    p.includes('program files\\microsoft\\') ||
    p.includes('program files (x86)\\microsoft\\')
  )
}

function estimateScore(totalMs: number, programCount: number): number {
  const timeScore = totalMs <= 60_000 ? 100
    : totalMs >= 120_000 ? 50
    : 100 - Math.floor((totalMs - 60_000) * 50 / 60_000)
  const countPenalty = Math.min(Math.max(0, programCount - 10) * 5, 30)
  return Math.max(10, timeScore - countPenalty)
}

function buildScoreReasonLines(totalMs: number | null, events: ProgramEvent[]): string[] {
  const lines: string[] = []
  if (totalMs == null) return lines

  if (totalMs <= 60_000) {
    lines.push(`부팅 시간 ${fmtMs(totalMs)} — 빠름 (감점 없음)`)
  } else if (totalMs >= 120_000) {
    const pen = 50
    lines.push(`부팅 시간 ${fmtMs(totalMs)} — 느림 (-${pen}점)`)
  } else {
    const pen = Math.round((totalMs - 60_000) * 50 / 60_000)
    lines.push(`부팅 시간 ${fmtMs(totalMs)} (-${pen}점)`)
  }

  const slowCount = events.filter(e => e.status === 'slow').length
  const failedCount = events.filter(e => e.status === 'failed').length
  const countPenalty = events.length > 10 ? Math.min((events.length - 10) * 5, 30) : 0

  if (slowCount > 0) lines.push(`느린 프로그램 ${slowCount}개 (-${slowCount * 5}점)`)
  if (failedCount > 0) lines.push(`실패한 프로그램 ${failedCount}개 (추가 감점)`)
  if (countPenalty > 0) lines.push(`시작프로그램 ${events.length}개 — 10개 초과 (-${countPenalty}점)`)
  if (lines.length === 1 && totalMs <= 60_000) lines.push('모든 프로그램이 정상 동작했습니다.')

  return lines
}

function fmtMs(ms: number | null): string {
  if (ms == null) return '—'
  if (ms < 1000) return `${ms}ms`
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`
  return `${Math.floor(ms / 60_000)}m ${Math.round((ms % 60_000) / 1000)}s`
}

function fmtTime(isoStr: string): string {
  try {
    return new Date(isoStr).toLocaleTimeString('ko-KR', { hour: '2-digit', minute: '2-digit' })
  } catch {
    return '—'
  }
}

function scoreColor(score: number): string {
  if (score >= 80) return '#1d9e75'
  if (score >= 50) return '#e09a30'
  return '#e05050'
}
