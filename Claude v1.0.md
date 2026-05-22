# BootReady v1.0 — 개발 여정

> git 커밋 히스토리 기반으로 재구성한 v1.0까지의 과정.

---

## 시작 (Initial Scaffold)

`88f1127` — **boot-core (Rust) + bootready-ui (Electron + React)**

처음 구조는 전형적인 Electron 앱.
- boot-core: Rust로 Windows 시작프로그램 감시 → SQLite 저장
- bootready-ui: Electron + React로 팝업 UI

---

## Electron 시절 (v0.1.x 초기)

`6f64446` → `507769a` → `def720b`

- 트레이 아이콘 (ring-progress 형태)
- 팝업 UI, 타임라인, 부팅 히스토리, 설정 화면 기본 골격 완성
- 아이콘 캐시, show.signal 감시 로직 추가
- GitHub Actions 릴리즈 파이프라인 구축

`df3a7de` → `bf2ac13` — **v0.1.5**

트레이 아이콘, 시작 동작, 언인스톨 정리 마무리.

---

## 핵심 전환점: Electron → Tauri v2

`a0bbb3c` → `c59ac80` — **80MB → 2.4MB**

Electron의 번들 크기 문제로 Tauri v2로 전면 마이그레이션.  
단순한 트레이 유틸에 Electron은 과했다.

- Named Pipe IPC 제거 → DB 직접 읽기로 단순화
- 단일 프로세스 구조로 변경
- 번들 크기 97% 감소

---

## Tauri 안정화 (v0.1.6)

`fc52907` → `6c03321` → `7dbb2b1` → `09f3b2b` → `50560fe`

- 트레이 아이콘 단일 인스턴스 가드
- NSIS installMode: currentUser
- 창 위치, 종료, 우클릭 메뉴
- boot-core 자동 실행 연동
- 창 크기 360×480 → 420×560
- 파일 아이콘 기능 추가 시도

---

## 트레이 토글 고생기 (v0.1.6 → v0.1.7)

`7f89f8d` → `582a246` → `356a6da` → `1625130`

트레이 아이콘 좌클릭 show/hide 토글 하나 잡는 데 커밋 4개 소요.

- 클릭 시 열리자마자 focus-lost로 닫히는 문제
- mouse-down 시점 visibility 추적으로 해결 시도
- 결국 revert 후 다른 방식으로 안정화

---

## 아이콘 기능 제거 (v0.1.7)

`57d04bc` → `ea9dd07` → `985f911`

SHGetFileInfoW COM 초기화 문제, 환경변수 미확장, exe_path 잘림 등  
아이콘 추출이 불안정해서 기능 전체 제거.  
의존성 정리, Electron 잔재 완전 제거.

---

## startup URL 기능 (v0.1.8)

`b952dae` → `a9cf0cd`

부팅 완료 시 지정한 URL을 브라우저로 자동 오픈.  
설정 화면에 URL 목록 관리 UI 추가.

---

## v1.0 릴리즈

boot-core.exe와 bootready.exe를 단일 setup.exe로 번들링.  
설치 후 즉시 동작 확인 완료.

---

## 기술 스택 최종

| | |
|---|---|
| 감시 프로세스 | Rust (boot-core) |
| UI 프레임워크 | Tauri v2 + React + Vite |
| DB | SQLite (rusqlite) |
| 인스톨러 | NSIS (currentUser) |
| 번들 크기 | ~2.4MB |
