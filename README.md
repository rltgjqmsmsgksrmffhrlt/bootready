# BootReady

Windows PC 부팅 후 모든 시작프로그램이 완료된 순간을 감지하여 시스템 트레이에 조용히 아이콘을 표시하는 경량 유틸리티.

## 구조

```
bootready/
├── boot-core/          # Rust — 부팅 시 자동 실행, 프로세스 감시, SQLite 기록
└── bootready-ui/       # Tauri v2 + React — 트레이 아이콘, 팝업 UI
```

## 개발 환경 준비

### 1. Rust 설치
```powershell
winget install Rustlang.Rustup
rustup toolchain install stable-x86_64-pc-windows-msvc
```

### 2. Node.js 설치
```powershell
winget install OpenJS.NodeJS.LTS
```

## boot-core 빌드

```powershell
cd boot-core
cargo build --release       # target/release/boot-core.exe
```

## bootready-ui 개발

```powershell
cd bootready-ui
npm install
npm run dev                 # Tauri + Vite 개발 서버
```

### 릴리스 빌드
```powershell
npm run build               # src-tauri/target/release/bundle/nsis/BootReady_{version}_x64-setup.exe
```

## 아키텍처

```
[부팅]
  boot-core.exe (Rust)
    ├── 레지스트리 + 시작폴더에서 시작프로그램 목록 수집
    ├── 500ms 간격으로 프로세스 폴링
    ├── 완료 감지 → SQLite에 세션/이벤트 기록
    └── show.signal 파일로 UI에 알림

[사용자 클릭 / 부팅 완료]
  bootready-ui.exe (Tauri v2)
    ├── SQLite 직접 읽기
    ├── show.signal 감시 → 자동 팝업
    └── React UI: 타임라인 / 이력 / 점수 / 설정
```

## 주요 경로

| 파일 | 경로 |
|------|------|
| DB | `%APPDATA%\BootReady\data.db` |
| 설정 | `%APPDATA%\BootReady\config.json` |
| 아이콘 캐시 | `%APPDATA%\BootReady\icons\` |

## 로드맵

| Phase | 상태 | 내용 |
|---|---|---|
| Phase 1 (MVP) | ✅ 완료 | boot-core + 팝업 + 타임라인 |
| Phase 2 | ✅ 완료 | 부팅 점수, 이력 그래프, 앱 아이콘 |
| Phase 3 | 📋 예정 | 비활성화 추천, 다국어, Enterprise |
