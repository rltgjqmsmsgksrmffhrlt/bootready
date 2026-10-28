import React, { useCallback, useEffect, useRef, useState } from 'react'
import ReactDOM from 'react-dom/client'
import '../src/styles/globals.css'
import './promo.css'
import trayIcon from '../assets/icon-32.png'
import { readUrls } from './mockApi'

const TOTAL = 7

// ───────── 공통 조각 ─────────

function Title({ children }: { children: React.ReactNode }) {
  return <h1 className="title">{children}</h1>
}

function Pager({ n }: { n: number }) {
  return <div className="pager">{n} / {TOTAL}</div>
}

function Cursor({ style }: { style: React.CSSProperties }) {
  return (
    <svg className="cursor" viewBox="0 0 24 24" style={style}>
      <path d="M4 2 L4 19 L8.5 15 L11.5 21.5 L14 20.4 L11 14 L17 14 Z" fill="#e8e6dc" stroke="#1a1a1a" strokeWidth="1.2" strokeLinejoin="round" />
    </svg>
  )
}

function Desktop({ tall, children }: { tall?: boolean; children?: React.ReactNode }) {
  return (
    <div className={`desktop${tall ? ' tall' : ''}`}>
      <div className="dicons">
        <div className="dicon"><i />문서</div>
        <div className="dicon"><i />사진</div>
        <div className="dicon"><i />다운로드</div>
      </div>
      {children}
      <div className="taskbar">
        <span className="tray-dot" />
        <span className="tray-dot" />
        <img src={trayIcon} alt="" />
      </div>
    </div>
  )
}

function FakeBrowser({ url, style, typing, className }: { url?: string; style: React.CSSProperties; typing?: string; className?: string }) {
  return (
    <div className={`fb ${className ?? ''}`} style={style}>
      <div className="fb-top">
        <div className="fb-dots"><i /><i /><i /></div>
        <div className="fb-addr">
          {typing ? <span className="type">{typing}</span> : (url ?? '').replace(/^https?:\/\//, '')}
        </div>
      </div>
      <div className="fb-body"><i /><i /><i /><i /></div>
    </div>
  )
}

// 실제 App 컴포넌트가 돌아가는 앱 창. 높이는 실제 앱처럼 앱이 알려 주는 값을 따른다.
function AppFrame({ route, width = 420, initialHeight = 560, onClose }: { route: string; width?: number; initialHeight?: number; onClose?: () => void }) {
  const [h, setH] = useState(initialHeight)
  const ref = useRef<HTMLIFrameElement>(null)
  useEffect(() => {
    const fn = (e: MessageEvent) => {
      if (e.source !== ref.current?.contentWindow) return
      if (e.data?.type === 'height') setH(Math.max(320, e.data.h))
      if (e.data?.type === 'close') onClose?.()
    }
    window.addEventListener('message', fn)
    return () => window.removeEventListener('message', fn)
  }, [onClose])
  return <iframe ref={ref} className="appframe" title="BootReady" src={`./app.html?route=${route}`} width={width} height={h} />
}

// ───────── 슬라이드 ─────────

function S1() {
  return (
    <>
      <Pager n={1} />
      <Title>PC 켜자마자 여는 그 사이트,<br /><span className="hl">아직도 직접</span> 여시나요?</Title>
      <Desktop><Cursor style={{ left: 300, top: 200 }} /></Desktop>
      <div className="swipe">넘겨 보세요 →</div>
    </>
  )
}

function S2() {
  return (
    <>
      <Pager n={2} />
      <Title>부팅 끝나길 기다렸다가,<br /><span className="hl">주소창에 또 치고.</span></Title>
      <div style={{ position: 'relative', width: 760, height: 380 }}>
        <FakeBrowser typing="mail.example.com" style={{ left: 0, top: 0, width: 760, height: 380 }} />
      </div>
    </>
  )
}

function S3() {
  const layer = (i: number): React.CSSProperties => ({ left: i * 46, top: (2 - i) * 46, width: 640, height: 320, opacity: 1 - (2 - i) * 0.32 })
  return (
    <>
      <Pager n={3} />
      <Title>내일 아침도 <span className="hl">똑같이.</span></Title>
      <div style={{ position: 'relative', width: 760, height: 410 }}>
        {[0, 1, 2].map(i => (
          <FakeBrowser key={i} typing="mail.example.com" style={layer(i)} />
        ))}
      </div>
    </>
  )
}

function S4() {
  return (
    <>
      <Pager n={4} />
      <Title>“<span className="hl">켜졌나?</span> 아직인가?”</Title>
      <Desktop><div className="wait" style={{ left: 300, top: 200 }} /></Desktop>
    </>
  )
}

function S5() {
  const scale = 1.3
  return (
    <>
      <Pager n={5} />
      <Title>한 번 <span className="hl">등록해 두면.</span></Title>
      <div style={{ width: 420 * scale, height: 560 * scale }}>
        <div style={{ transform: `scale(${scale})`, transformOrigin: 'top left', width: 420 }}>
          <AppFrame route="settings" />
        </div>
      </div>
    </>
  )
}

function S6({ active, run }: { active: boolean; run: number }) {
  const [phase, setPhase] = useState<'idle' | 'booting' | 'done'>('idle')
  const [urls, setUrls] = useState<string[]>([])
  const [popup, setPopup] = useState(true)

  useEffect(() => {
    if (!active) { setPhase('idle'); return }
    setUrls(readUrls())
    setPopup(true)
    setPhase('booting')
    // 앱은 부팅 완료 시점에 팝업 호출과 URL 열기를 연달아 실행한다(watcher.rs finalize).
    const t = setTimeout(() => setPhase('done'), 2200)
    return () => clearTimeout(t)
  }, [active, run])

  return (
    <>
      <Pager n={6} />
      <Title>부팅이 끝나면,<br /><span className="hl">등록한 페이지가</span> 열립니다.</Title>
      <Desktop tall>
        {phase === 'booting' && <div className="wait" style={{ left: 420, top: 260 }} />}
        {phase === 'done' && urls.slice(0, 3).map((u, i) => (
          <FakeBrowser key={u + i} className="pop" url={u} style={{ left: 120 + i * 44, top: 28 + i * 40, width: 520, height: 300, animationDelay: `${i * 140}ms` }} />
        ))}
        {phase === 'done' && popup && (
          <div style={{ position: 'absolute', right: 16, bottom: 64, transform: 'scale(.8)', transformOrigin: 'bottom right' }}>
            <AppFrame route="popup" onClose={() => setPopup(false)} />
          </div>
        )}
      </Desktop>
    </>
  )
}

function S7() {
  return (
    <>
      <Pager n={7} />
      <Title><span className="hl">프로필 링크</span>에서<br />받을 수 있습니다.</Title>
      <div className="brand"><img src={trayIcon} alt="" />BootReady</div>
    </>
  )
}

// ───────── 캐러셀 ─────────

const NOTES = [
  '확인 필요: 7장 받는 곳(프로필 링크가 가리킬 GitHub Releases 또는 스토어)은 아직 정해지지 않음.',
  '가짜 값: 6장 팝업의 부팅 점수 · 시간 · 프로그램 이름은 예시. 실제 PC 캡처로 교체해야 함. 앱 UI는 실제 컴포넌트.',
  '실제 앱과 다른 점: 바탕화면, 브라우저 창, 커서는 연출. 팝업은 실제 앱의 위치(우하단)와 같지만 화면 비율상 0.8배.',
  '실제 조건: 앱은 로그온 후 10분 이내에만 팝업과 URL을 연다(watcher.rs). 이 시뮬레이션은 항상 연다.',
  '5장에서 URL을 추가/수정/삭제하면 6장에서 그대로 열린다(최대 3개까지만 그림).',
]

function Carousel() {
  const [i, setI] = useState(0)
  const [run, setRun] = useState(0)
  const [scale, setScale] = useState(1)
  const [showNotes, setShowNotes] = useState(false)
  const vp = useRef<HTMLDivElement>(null)
  const drag = useRef<number | null>(null)

  const go = useCallback((d: number) => setI(v => Math.min(TOTAL - 1, Math.max(0, v + d))), [])

  useEffect(() => {
    const fn = (e: KeyboardEvent) => {
      if ((e.target as HTMLElement)?.tagName === 'INPUT') return
      if (e.key === 'ArrowRight') go(1)
      if (e.key === 'ArrowLeft') go(-1)
    }
    window.addEventListener('keydown', fn)
    return () => window.removeEventListener('keydown', fn)
  }, [go])

  useEffect(() => {
    const el = vp.current
    if (!el) return
    const ro = new ResizeObserver(() => setScale(Math.min(el.clientWidth / 1080, el.clientHeight / 1080)))
    ro.observe(el)
    return () => ro.disconnect()
  }, [])

  const slides = [<S1 />, <S2 />, <S3 />, <S4 />, <S5 />, <S6 active={i === 5} run={run} />, <S7 />]

  return (
    <div className="shell">
      <div className="toolbar">
        <button onClick={() => go(-1)} disabled={i === 0}>← 이전</button>
        <button onClick={() => go(1)} disabled={i === TOTAL - 1}>다음 →</button>
        <div className="dots">
          {slides.map((_, k) => <button key={k} className={`dot${k === i ? ' on' : ''}`} onClick={() => setI(k)} aria-label={`${k + 1}장`} />)}
        </div>
        {i === 5 && <button onClick={() => setRun(r => r + 1)}>다시 부팅</button>}
        <span className="grow" />
        <button onClick={() => setShowNotes(v => !v)}>{showNotes ? '메모 닫기' : '확인 필요 · 가짜 값 메모'}</button>
      </div>
      {showNotes && <div className="notes">{NOTES.map(n => <div key={n}>• {n}</div>)}</div>}
      <div
        className="viewport"
        ref={vp}
        onPointerDown={e => { if ((e.target as HTMLElement).closest('iframe')) return; drag.current = e.clientX }}
        onPointerUp={e => {
          if (drag.current == null) return
          const dx = e.clientX - drag.current
          drag.current = null
          if (Math.abs(dx) > 60) go(dx < 0 ? 1 : -1)
        }}
      >
        <div style={{ width: 1080 * scale, height: 1080 * scale, flex: 'none' }}>
          <div className="stage" style={{ transform: `scale(${scale})`, transformOrigin: 'top left' }}>
            {slides.map((s, k) => <div key={k} className={`slide${k === i ? ' on' : ''}`}>{s}</div>)}
          </div>
        </div>
      </div>
    </div>
  )
}

ReactDOM.createRoot(document.getElementById('root')!).render(<Carousel />)
