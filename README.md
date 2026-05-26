# BootReady

Windows PC 부팅 후 모든 시작프로그램이 완료된 순간을 감지하여 시스템 트레이에 조용히 아이콘을 표시하는 경량 유틸리티.

## 구조

```
bootready/
├── bootready-ui/       # Tauri v2 + React — 단일 프로세스 (백그라운드 감시 + 트레이 + UI)
└── legacy/
    └── boot-core/      # (deprecated) 1.0.x까지 사용하던 별도 프로세스. 히스토리 보존용
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

## 개발

```powershell
cd bootready-ui
npm install
npm run dev        # Tauri + Vite 개발 서버
```

### 릴리스 빌드
```powershell
npm run build      # src-tauri/target/release/bundle/nsis/BootReady_{version}_x64-setup.exe
```

## 아키텍처 (v1.1.0+)

단일 프로세스로 통합:

```
[부팅]
  BootReady.exe (Tauri v2 + Rust)
    ├── 레지스트리 + 시작폴더에서 시작프로그램 목록 수집
    ├── 500ms 간격으로 프로세스 폴링
    ├── 완료 감지 → SQLite에 세션/이벤트 기록
    ├── 부팅 완료 시 등록된 URL 자동 오픈 (uptime 가드 포함)
    ├── 앱 시작 시 GitHub에서 업데이트 확인 (1회)
    └── show.signal → 자동 팝업

[사용자 클릭]
  React UI
    ├── 타임라인 / 이력 / 점수
    └── 설정 (자동시작, URL 등록, 감지 옵션)
```

## 주요 경로

| 파일 | 경로 |
|------|------|
| DB | `%APPDATA%\BootReady\data.db` |
| 설정 | `%APPDATA%\BootReady\config.json` |
| 시그널 | `%APPDATA%\BootReady\show.signal` |

## 로드맵

| Phase | 상태 | 내용 |
|---|---|---|
| Phase 1 (MVP) | ✅ 완료 | 시작프로그램 감시 + 팝업 + 타임라인 |
| Phase 2 | ✅ 완료 | 부팅 점수, 이력 그래프, 설정 화면 |
| Phase 3 | ✅ 완료 | 단일 프로세스 통합, URL 자동 오픈, 자동 업데이트 알림 |
| Phase 4 | 📋 예정 | 부팅 트렌드 경고 (최근 N회 평균 대비 느려지면 알림) |
