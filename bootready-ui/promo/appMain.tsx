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
