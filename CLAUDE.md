# BootReady — 프로젝트 가이드

## 버전 관리
**현재 버전: 0.1.7**

버전 변경 시 반드시 아래 4곳을 함께 수정:
1. `bootready-ui/package.json` → `"version"`
2. `bootready-ui/src-tauri/Cargo.toml` → `"version"`
3. `bootready-ui/src-tauri/tauri.conf.json` → `"version"`
4. `bootready-ui/src/components/Settings.tsx` → `InfoRow label="버전"` 의 value

## 빌드 방법

```bash
cd bootready-ui
npm run build
```

결과물: `bootready-ui/src-tauri/target/release/bundle/nsis/BootReady_{version}_x64-setup.exe`

## 릴리즈 방법

```bash
gh release create v{version} \
  "bootready-ui/src-tauri/target/release/bundle/nsis/BootReady_{version}_x64-setup.exe" \
  --title "BootReady v{version}" --latest
```

## 아키텍처

- **boot-core** (Rust): 부팅 시 시작프로그램 감시, SQLite 저장, Named Pipe IPC
- **bootready-ui** (Tauri v2 + React): 팝업 UI, DB 직접 읽기, show.signal 감시, 트레이 아이콘

## 주요 경로 (런타임)

| 파일 | 경로 |
|------|------|
| DB | `%APPDATA%\BootReady\data.db` |
| 설정 | `%APPDATA%\BootReady\config.json` |
| 시그널 | `%APPDATA%\BootReady\show.signal` |
| 아이콘 캐시 | `%APPDATA%\BootReady\icons\` |
| boot-core 바이너리 | `%APPDATA%\BootReady\boot-core.exe` |

## 브랜치 전략

- `master`: 릴리즈 브랜치
- 기능 개발: `feature/xxx` 브랜치 → master 머지

## get_file_icon 구현 상세

`bootready-ui/src-tauri/src/main.rs`에 구현됨.
- `SHGetFileInfoW` (Windows Shell API) 로 네이티브 아이콘 추출
- **COM 초기화 필수**: `CoInitializeEx(COINIT_APARTMENTTHREADED)` → `CoUninitialize()`
- **환경변수 확장**: `ExpandEnvironmentStringsW` → `%windir%` 같은 경로 처리
- 추출 성공 시 PNG로 인코딩 후 `%APPDATA%\BootReady\icons\{hash}.png` 캐싱
- 필요 Cargo features: `Win32_System_Com`, `Win32_System_Environment`

## TODO

> ⚠️ 세션 끊기기 전 반드시 업데이트.

### 현재 진행 중
- [ ] 0.1.6 설치 후 아이콘 표시 실제 동작 확인 필요
  - `%APPDATA%\BootReady\icons\` 폴더 생성 여부로 성공 판단

### 다음 작업
- [ ] exe_path 잘림 문제 조사 (boot-core에서 일부 경로가 중간에 잘려 저장됨)

### 완료
- [x] `get_file_icon`: SHGetFileInfoW 네이티브 구현 (PowerShell 방식 제거)
- [x] COM 초기화 + 환경변수 확장 추가 (2026-05-22, v0.1.6)
- [x] 트레이 아이콘 클릭 토글 (show/hide)
- [x] 창 포커스 잃으면 자동 숨김
- [x] 부팅 점수 + 이력 그래프
- [x] 설정 화면 자동시작 토글
