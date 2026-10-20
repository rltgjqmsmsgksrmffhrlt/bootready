import { useState, useEffect, useRef } from 'react'
import styles from './Settings.module.css'

interface Props {
  onBack: () => void
}

interface SettingsState {
  autostart: boolean
  idleSeconds: number
  slowThresholdMs: number
  startupUrls: string[]
}

const DEFAULT_SETTINGS: SettingsState = {
  autostart: true,
  idleSeconds: 5,
  slowThresholdMs: 30000,
  startupUrls: [],
}

export default function Settings({ onBack }: Props) {
  const [settings, setSettings] = useState<SettingsState>(DEFAULT_SETTINGS)
  const [saved, setSaved] = useState(false)
  const [urlInput, setUrlInput] = useState('')
  const urlInputRef = useRef<HTMLInputElement>(null)
  const [editingIndex, setEditingIndex] = useState<number | null>(null)
  const [editInput, setEditInput] = useState('')

  useEffect(() => {
    async function load() {
      const autostart = await window.api?.getAutostart?.() ?? true
      const config = await window.api?.loadConfig?.() ?? {}
      setSettings({
        autostart,
        idleSeconds: (config.idleSeconds as number) ?? DEFAULT_SETTINGS.idleSeconds,
        slowThresholdMs: (config.slowThresholdMs as number) ?? DEFAULT_SETTINGS.slowThresholdMs,
        startupUrls: (config.startup_urls as string[]) ?? [],
      })
    }
    load()
  }, [])

  async function save() {
    try {
      await window.api?.setAutostart?.(settings.autostart)
      await window.api?.saveConfig?.({
        idleSeconds: settings.idleSeconds,
        slowThresholdMs: settings.slowThresholdMs,
        startup_urls: settings.startupUrls,
      })
      setSaved(true)
      setTimeout(() => setSaved(false), 2000)
    } catch {}
  }

  // URL 목록은 변경 즉시 저장 — 기존 config를 읽어 startup_urls만 교체 (다른 키 보존).
  // 연속 변경 시 저장 순서가 꼬이지 않도록 이전 저장이 끝난 뒤 실행.
  const urlSaveChain = useRef<Promise<unknown>>(Promise.resolve())

  function updateUrls(next: string[]) {
    setSettings(s => ({ ...s, startupUrls: next }))
    urlSaveChain.current = urlSaveChain.current.then(async () => {
      try {
        const config = await window.api?.loadConfig?.() ?? {}
        await window.api?.saveConfig?.({ ...config, startup_urls: next })
      } catch (e) {
        console.error('failed to save startup urls:', e)
      }
    })
  }

  function normalizeUrl(raw: string): string | null {
    const url = raw.trim()
    if (!url) return null
    return url.startsWith('http') ? url : `https://${url}`
  }

  function addUrl() {
    const normalized = normalizeUrl(urlInput)
    if (!normalized) return
    if (settings.startupUrls.includes(normalized)) return
    updateUrls([...settings.startupUrls, normalized])
    setUrlInput('')
    urlInputRef.current?.focus()
  }

  function removeUrl(i: number) {
    if (editingIndex === i) setEditingIndex(null)
    updateUrls(settings.startupUrls.filter((_, idx) => idx !== i))
  }

  function startEdit(i: number) {
    setEditingIndex(i)
    setEditInput(settings.startupUrls[i])
  }

  function commitEdit() {
    if (editingIndex === null) return
    const normalized = normalizeUrl(editInput)
    if (!normalized) return
    const duplicate = settings.startupUrls.some((u, idx) => idx !== editingIndex && u === normalized)
    if (duplicate) return
    if (normalized !== settings.startupUrls[editingIndex]) {
      updateUrls(settings.startupUrls.map((u, idx) => (idx === editingIndex ? normalized : u)))
    }
    setEditingIndex(null)
  }

  return (
    <div className={styles.root}>
      <div className={styles.header}>
        <button className={styles.backBtn} onClick={onBack}>
          ← 돌아가기
        </button>
        <span className={styles.headerTitle}>설정</span>
      </div>

      <div className={styles.body}>
        {/* 자동 실행 */}
        <Section title="시작 설정">
          <ToggleRow
            label="Windows 시작 시 자동 실행"
            hint="BootReady를 시작프로그램으로 등록합니다"
            value={settings.autostart}
            onChange={(v) => setSettings((s) => ({ ...s, autostart: v }))}
          />
        </Section>

        {/* 부팅 완료 시 열 URL */}
        <Section title="부팅 완료 시 열 URL">
          <div className={styles.urlSection}>
            <div className={styles.urlHint}>부팅이 완료되면 아래 URL을 브라우저로 자동으로 엽니다. 추가·수정·삭제는 바로 저장됩니다.</div>
            {settings.startupUrls.length > 0 && (
              <ul className={styles.urlList}>
                {settings.startupUrls.map((url, i) => (
                  <li key={i} className={styles.urlItem}>
                    {editingIndex === i ? (
                      <>
                        <input
                          className={styles.urlEditInput}
                          type="text"
                          autoFocus
                          value={editInput}
                          onChange={e => setEditInput(e.target.value)}
                          onKeyDown={e => {
                            if (e.key === 'Enter') commitEdit()
                            else if (e.key === 'Escape') setEditingIndex(null)
                          }}
                        />
                        <button className={styles.urlEditBtn} onClick={commitEdit}>확인</button>
                        <button className={styles.urlEditBtn} onClick={() => setEditingIndex(null)}>취소</button>
                      </>
                    ) : (
                      <>
                        <span className={styles.urlText} title={url}>{url}</span>
                        <button className={styles.urlEditBtn} onClick={() => startEdit(i)} title="수정">수정</button>
                        <button className={styles.urlRemoveBtn} onClick={() => removeUrl(i)} title="삭제">✕</button>
                      </>
                    )}
                  </li>
                ))}
              </ul>
            )}
            <div className={styles.urlInputRow}>
              <input
                ref={urlInputRef}
                className={styles.urlInput}
                type="text"
                placeholder="https://example.com"
                value={urlInput}
                onChange={e => setUrlInput(e.target.value)}
                onKeyDown={e => e.key === 'Enter' && addUrl()}
              />
              <button className={styles.urlAddBtn} onClick={addUrl}>추가</button>
            </div>
          </div>
        </Section>

        {/* 감지 설정 */}
        <Section title="감지 설정">
          <SliderRow
            label="완료 대기 시간"
            hint="마지막 프로세스 이후 N초간 신규 프로세스가 없으면 완료로 판단"
            value={settings.idleSeconds}
            min={3}
            max={30}
            step={1}
            format={(v) => `${v}초`}
            onChange={(v) => setSettings((s) => ({ ...s, idleSeconds: v }))}
          />
          <SliderRow
            label="느린 프로그램 기준"
            hint="이 시간 이상 걸리면 '느림' 뱃지를 표시합니다"
            value={settings.slowThresholdMs / 1000}
            min={5}
            max={120}
            step={5}
            format={(v) => `${v}초`}
            onChange={(v) => setSettings((s) => ({ ...s, slowThresholdMs: v * 1000 }))}
          />
        </Section>

        {/* 버전 정보 */}
        <Section title="정보">
          <InfoRow label="버전" value="1.3.1" />
          <InfoRow label="DB 위치" value="%APPDATA%\BootReady\data.db" mono />
          <InfoRow label="만든 사람" value="petit prin" />
        </Section>
      </div>

      <div className={styles.footer}>
        <button className={styles.saveBtn} onClick={save}>
          {saved ? '✓ 저장됨' : '저장'}
        </button>
        <button
          className={styles.quitBtn}
          onClick={() => window.api?.quitApp?.()}
        >
          앱 종료
        </button>
      </div>
    </div>
  )
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className={styles.section}>
      <div className={styles.sectionTitle}>{title}</div>
      <div className={styles.sectionBody}>{children}</div>
    </div>
  )
}

function ToggleRow({
  label,
  hint,
  value,
  onChange,
}: {
  label: string
  hint: string
  value: boolean
  onChange: (v: boolean) => void
}) {
  return (
    <div className={styles.row}>
      <div className={styles.rowText}>
        <span className={styles.rowLabel}>{label}</span>
        <span className={styles.rowHint}>{hint}</span>
      </div>
      <button
        className={`${styles.toggle} ${value ? styles.toggleOn : ''}`}
        onClick={() => onChange(!value)}
        role="switch"
        aria-checked={value}
      >
        <div className={styles.toggleThumb} />
      </button>
    </div>
  )
}

function SliderRow({
  label,
  hint,
  value,
  min,
  max,
  step,
  format,
  onChange,
}: {
  label: string
  hint: string
  value: number
  min: number
  max: number
  step: number
  format: (v: number) => string
  onChange: (v: number) => void
}) {
  return (
    <div className={styles.row}>
      <div className={styles.rowText}>
        <div className={styles.rowLabelRow}>
          <span className={styles.rowLabel}>{label}</span>
          <span className={styles.rowValue}>{format(value)}</span>
        </div>
        <span className={styles.rowHint}>{hint}</span>
      </div>
      <input
        type="range"
        className={styles.slider}
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
      />
    </div>
  )
}

function InfoRow({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className={styles.infoRow}>
      <span className={styles.infoLabel}>{label}</span>
      <span className={`${styles.infoValue} ${mono ? styles.mono : ''}`}>{value}</span>
    </div>
  )
}
