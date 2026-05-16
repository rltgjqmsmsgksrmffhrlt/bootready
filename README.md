# BootReady

Windows PC 부팅 후 모든 시작프로그램이 완료된 순간을 감지하여 시스템 트레이에 조용히 아이콘을 표시하는 경량 유틸리티.

## 구조

```
bootready/
├── boot-core/          # Rust — 부팅 시 자동 실행, 프로세스 감시, 트레이 아이콘
└── bootready-ui/       # Electron + React — 클릭 시 팝업 UI
```

## 개발 환경 준비

### 1. Rust 설치
```powershell
winget install Rustlang.Rustup
# 재시작 후
rustup toolchain install stable-x86_64-pc-windows-msvc
```

### 2. Node.js 설치
```powershell
winget install OpenJS.NodeJS.LTS
```

## boot-core 빌드

```powershell
cd boot-core
cargo build                 # 디버그 빌드
cargo build --release       # 릴리스 빌드 (target/release/boot-core.exe)
```

### 개발 중 실행 (콘솔 로그 확인)
```powershell
$env:RUST_LOG="debug"
cargo run
```

## bootready-ui 개발

```powershell
cd bootready-ui
npm install
npm run electron:dev        # Electron + Vite 개발 서버 동시 실행
```

### UI 빌드
```powershell
npm run build               # dist/ + dist-electron/ 생성 후 인스톨러 패키징
```

## 아키텍처 요약

```
[부팅]
  boot-core.exe (Rust)
    ├── 레지스트리 + 시작폴더에서 시작프로그램 목록 수집
    ├── 500ms 간격으로 프로세스 목록 폴링
    ├── 완료 감지 → SQLite에 세션/이벤트 기록
    ├── 트레이에 초록 아이콘 표시
    └── Named Pipe 서버 (\\.\pipe\bootready)

[사용자 클릭]
  bootready-ui.exe (Electron)
    ├── SQLite 직접 읽기 (read-only)
    ├── Named Pipe로 boot-core 상태 조회 (선택)
    └── React UI: 팝업 / 타임라인 / 설정
```

## DB 위치

```
%APPDATA%\BootReady\data.db
```

## 개발 로드맵

| Phase | 상태 | 내용 |
|---|---|---|
| Phase 1 (MVP) | 🚧 개발 중 | boot-core + 팝업 + 타임라인 |
| Phase 2 | 📋 예정 | 부팅 점수, 이력 그래프, Pro 구독 |
| Phase 3 | 📋 예정 | 비활성화 추천, 다국어, Enterprise |
