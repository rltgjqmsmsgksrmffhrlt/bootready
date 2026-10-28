import React, { useCallback, useEffect, useRef, useState } from 'react'
import ReactDOM from 'react-dom/client'
import '../src/styles/globals.css'
import './promo.css'
import { readUrls } from './mockApi'

// Figma "Claude" 섹션(6장 구성)과 같은 글, 같은 좌표. 앱 화면은 실제 컴포넌트를 iframe으로 렌더한다.
const TOTAL = 6
const pad = (n: number) => String(n).padStart(2, '0')

type P = React.CSSProperties
const A = ({ top, left, cls, style, children }: { top: number; left?: number; cls?: string; style?: P; children: React.ReactNode }) => (
  <div className={`abs ${cls ?? ''}`} style={{ top, left, ...style }}>{children}</div>
)

function Chrome({ n, label }: { n: number; label?: string }) {
  return (
    <>
      {n > 1 && <A top={54} cls="brand">BootReady / 사용 기록</A>}
      <A top={54} cls="pager">{`${pad(n)}     /     ${pad(TOTAL)}`}</A>
      {label && <A top={138} cls="label">{label}</A>}
    </>
  )
}

// 앱 창을 iframe으로. 높이는 실제 앱처럼 앱이 알려 주는 값을 따르고, 배율은 바깥에서만 준다.
function AppFrame({ route, scale = 1, initialHeight = 560, onClose }: { route: string; scale?: number; initialHeight?: number; onClose?: () => void }) {
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
  return (
    <div style={{ width: 420 * scale, height: h * scale }}>
      <iframe ref={ref} className="appframe" title="BootReady" src={`./app.html?route=${route}`} width={420} height={h}
        style={{ transform: `scale(${scale})`, transformOrigin: 'top left' }} />
    </div>
  )
}

function FakeBrowser({ url, style, delay }: { url: string; style: P; delay: number }) {
  return (
    <div className="fb pop" style={{ ...style, animationDelay: `${delay}ms` }}>
      <div className="fb-top"><div className="fb-dots"><i /><i /><i /></div><div className="fb-addr">{url.replace(/^https?:\/\//, '')}</div></div>
      <div className="fb-body"><i /><i /><i /><i /></div>
    </div>
  )
}

// ───────── 슬라이드 ─────────

function S1() {
  return (
    <>
      <Chrome n={1} />
      <A top={390} style={{ fontSize: 28, lineHeight: '34px' }} cls="t-g">제가 6개월 써보며</A>
      <A top={452} style={{ fontSize: 68, lineHeight: '84px' }} cls="t-w">PC를 켠 뒤,</A>
      <A top={550} style={{ fontSize: 58, lineHeight: '76px' }} cls="t-a">{'매일 하던 일을\n하나 줄였습니다.'}</A>
      <A top={960} cls="swipe">넘겨 보세요 →</A>
    </>
  )
}

function S2() {
  return (
    <>
      <Chrome n={2} label="매일 반복되는 일" />
      <A top={250} style={{ fontSize: 62, lineHeight: '78px' }} cls="t-w">PC를 켜고,</A>
      <A top={350} left={160} style={{ fontSize: 62, lineHeight: '78px' }} cls="t-w">기다렸다가,</A>
      <A top={450} style={{ fontSize: 48, lineHeight: '66px' }} cls="t-r">자주 쓰는 페이지를 열고.</A>
      <div className="rule" style={{ top: 620, width: 952 }} />
      <A top={680} style={{ fontSize: 25, lineHeight: '30px' }} cls="t-g">생각해 보니 이 순서를 매일 반복하고 있더라고요.</A>
    </>
  )
}

// 실제 설정 화면(Settings)의 'URL 등록' 카드만 보이도록 잘라 낸다. 안에서 추가, 수정, 삭제가 실제로 된다.
function S3() {
  const s = 1.9
  return (
    <>
      <Chrome n={3} label="한 번 등록하기" />
      <A top={200} style={{ fontSize: 56, lineHeight: '72px' }} cls="t-w">{'자주 여는 URL을\n미리 등록해 두면'}</A>
      <div className="demo clear" style={{ top: 405, height: 440 }}>
        <div style={{ position: 'absolute', left: 95, top: 0, width: 761, height: 440, overflow: 'hidden' }}>
          <div style={{ position: 'absolute', left: -19, top: -306 }}>
            <AppFrame route="settings&crop=url" scale={s} />
          </div>
        </div>
      </div>
      <div className="rule" style={{ top: 890, width: 952 }} />
      <A top={920} cls="note">별일은 아니지만, 매일 하는 일이라면 한 번 줄여볼 만했어요.</A>
    </>
  )
}

function S4({ active, run }: { active: boolean; run: number }) {
  const [phase, setPhase] = useState<'idle' | 'booting' | 'done'>('idle')
  const [urls, setUrls] = useState<string[]>([])
  const [popup, setPopup] = useState(true)
  useEffect(() => {
    if (!active) { setPhase('idle'); return }
    setUrls(readUrls()); setPopup(true); setPhase('booting')
    // 앱은 부팅 완료 시점에 팝업 호출과 URL 열기를 연달아 실행한다(watcher.rs finalize).
    const t = setTimeout(() => setPhase('done'), 2200)
    return () => clearTimeout(t)
  }, [active, run])
  return (
    <>
      <Chrome n={4} label="가장 편했던 기능" />
      <A top={190} style={{ fontSize: 58, lineHeight: '74px' }} cls="t-w">부팅이 완료되면,</A>
      <A top={275} style={{ fontSize: 47, lineHeight: '64px' }} cls="t-a">등록한 페이지가 열립니다.</A>
      <div className="demo" style={{ top: 405, height: 410 }}>
        <div className="scene">
          {phase === 'booting' && <div className="wait" style={{ left: 458, top: 150 }} />}
          {phase === 'done' && urls.slice(0, 3).map((u, i) => (
            <FakeBrowser key={u + i} url={u} delay={i * 140} style={{ left: 40 + i * 40, top: 24 + i * 34, width: 520, height: 250 }} />
          ))}
          {phase === 'done' && popup && (
            <div style={{ position: 'absolute', right: 14, bottom: 52 }}>
              <AppFrame route="popup" scale={0.62} onClose={() => setPopup(false)} />
            </div>
          )}
          <div className="taskbar" />
        </div>
      </div>
      <div className="rule" style={{ top: 860, width: 952 }} />
      <A top={900} cls="note">PC를 켠 직후(로그인 후 10분 이내)에만 열립니다.</A>
    </>
  )
}

function S5() {
  return (
    <>
      <Chrome n={5} label="함께 확인할 수 있는 것" />
      <A top={200} style={{ fontSize: 58, lineHeight: '74px' }} cls="t-w">부팅이 빨라지는 건 아니에요.</A>
      <A top={280} style={{ fontSize: 48, lineHeight: '64px' }} cls="t-w">부팅 점수와 프로그램별 소요 시간은</A>
      <A top={348} style={{ fontSize: 48, lineHeight: '64px' }} cls="t-a">확인할 수 있어요.</A>
      <div className="demo" style={{ top: 470, height: 340, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
        <AppFrame route="popup" scale={0.72} />
      </div>
      <div className="rule" style={{ top: 860, width: 952 }} />
      <A top={900} cls="note">시작프로그램을 켜고 끄는 기능은 없습니다.</A>
    </>
  )
}

function S6() {
  return (
    <>
      <Chrome n={6} label="마무리" />
      <A top={260} style={{ fontSize: 42, lineHeight: '58px' }} cls="t-r">저처럼 매일 같은 페이지를 연다면,</A>
      <A top={335} style={{ fontSize: 70, lineHeight: '88px' }} cls="t-a">한 번 써보세요.</A>
      <div className="rule" style={{ top: 510, width: 170 }} />
      <A top={570} style={{ fontSize: 52, lineHeight: '62px' }} cls="t-w">BootReady</A>
      <A top={650} style={{ fontSize: 23, lineHeight: '28px' }} cls="t-g">URL 자동 열기와 부팅 기록을 도와주는 Windows 10 / 11 유틸리티</A>
      <A top={790} style={{ fontSize: 24, lineHeight: '29px' }} cls="t-a">다운로드</A>
      <A top={836} style={{ fontSize: 18, lineHeight: '22px' }} cls="t-g">프로필 링크에서 받을 수 있습니다.</A>
    </>
  )
}

// ───────── 캐러셀 ─────────

const NOTES = [
  '글과 좌표는 Figma "Claude" 섹션(6장)과 같음. 폰트는 Figma와 같은 Noto Sans KR(미설치 시 대체 글꼴로 보임).',
  '실제 앱 컴포넌트: 3장 설정 화면(URL 추가, 수정, 삭제가 실제로 됨), 4, 5장 팝업. 3장에서 등록한 URL이 4장에서 그대로 열림(최대 3개까지만 그림).',
  '가짜 값: 4, 5장 팝업의 부팅 점수, 시간, 프로그램 이름은 예시. 발행 전 실제 PC 캡처로 교체해야 함.',
  '연출: 4장 바탕화면과 브라우저 창, 팝업 배율(0.62배)은 화면 구성을 위한 것. 브라우저는 로고 없이 글자만, 주소는 example.com.',
  '실제 앱 조건: 로그온 후 10분 이내에만 팝업과 URL이 열림(watcher.rs). 이 시뮬레이션은 항상 열림.',
  '확인 필요: 6장 받는 곳(프로필 링크가 가리킬 GitHub Releases 또는 스토어), 말하는 사람(만든 사람 표기 여부).',
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

  const slides = [<S1 />, <S2 />, <S3 />, <S4 active={i === 3} run={run} />, <S5 />, <S6 />]

  return (
    <div className="shell">
      <div className="toolbar">
        <button onClick={() => go(-1)} disabled={i === 0}>← 이전</button>
        <button onClick={() => go(1)} disabled={i === TOTAL - 1}>다음 →</button>
        <div className="dots">
          {slides.map((_, k) => <button key={k} className={`dot${k === i ? ' on' : ''}`} onClick={() => setI(k)} aria-label={`${k + 1}장`} />)}
        </div>
        {i === 3 && <button onClick={() => setRun(r => r + 1)}>다시 부팅</button>}
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
