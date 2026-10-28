import React from 'react'
import ReactDOM from 'react-dom/client'
import App from '../src/App'
import '../src/styles/globals.css'
import { installMockApi } from './mockApi'

// 실제 앱의 App 컴포넌트를 그대로 렌더한다. 바뀐 것은 window.api 뿐.
installMockApi()

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
)

// ?crop=url: 홍보물 3장에서 'URL 등록' 카드만 보이도록 앞뒤 섹션을 숨긴다(카드 자체는 그대로).
if (new URLSearchParams(location.search).get('crop') === 'url') {
  const hide = () => {
    const title = [...document.querySelectorAll('div')].find(el => el.children.length === 0 && el.textContent === '부팅 완료 시 열 URL')
    const section = title?.parentElement
    if (!section?.parentElement) return
    for (const sib of Array.from(section.parentElement.children)) {
      if (sib !== section) (sib as HTMLElement).style.visibility = 'hidden'
    }
  }
  new MutationObserver(hide).observe(document.getElementById('root')!, { childList: true, subtree: true })
}
