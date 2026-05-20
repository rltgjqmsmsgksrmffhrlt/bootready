import { useEffect, useState } from 'react'
import TrayPopup from './components/TrayPopup'
import Timeline from './components/Timeline'
import Settings from './components/Settings'
import History from './components/History'
import type { SessionWithEvents } from './types'

type Route = 'popup' | 'timeline' | 'settings' | 'history'

export default function App() {
  const [route, setRoute] = useState<Route>('popup')
  const [sessionData, setSessionData] = useState<SessionWithEvents | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    loadData()

    window.api?.onNavigate?.((r) => {
      if (r === '/timeline') setRoute('timeline')
      else if (r === '/settings') setRoute('settings')
      else setRoute('popup')
    })

    // 세션 완료 시 자동 갱신
    window.api?.onSessionUpdated?.(() => loadData())
  }, [])

  async function loadData() {
    try {
      const data = await window.api?.getLatestSession?.()
      setSessionData(data ?? null)
    } catch (e) {
      console.error('failed to load session data:', e)
    } finally {
      setLoading(false)
    }
  }

  if (loading) {
    return (
      <div style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        height: '100vh',
        color: 'var(--color-text-secondary)',
        fontSize: '12px',
        gap: '8px',
      }}>
        <Spinner />
        <span>데이터 로딩 중…</span>
      </div>
    )
  }

  return (
    <>
      {route === 'popup' && (
        <TrayPopup
          sessionData={sessionData}
          onOpenTimeline={() => setRoute('timeline')}
          onOpenSettings={() => setRoute('settings')}
          onOpenHistory={() => setRoute('history')}
          onClose={() => window.api?.closeWindow?.()}
        />
      )}
      {route === 'timeline' && (
        <Timeline
          sessionData={sessionData}
          onBack={() => setRoute('popup')}
        />
      )}
      {route === 'settings' && (
        <Settings onBack={() => setRoute('popup')} />
      )}
      {route === 'history' && (
        <History
          onBack={() => setRoute('popup')}
          onSelectSession={(data) => { setSessionData(data); setRoute('timeline') }}
        />
      )}
    </>
  )
}

function Spinner() {
  return (
    <div style={{
      width: 16,
      height: 16,
      border: '2px solid var(--color-border)',
      borderTopColor: 'var(--color-accent)',
      borderRadius: '50%',
      animation: 'spin 0.8s linear infinite',
    }} />
  )
}
