# BootReady — 프로젝트 가이드

## 버전 관리
**현재 버전: 0.1.5**

버전 변경 시 반드시 아래 두 곳을 함께 수정:
1. `bootready-ui/package.json` → `"version"`
2. `bootready-ui/src-tauri/tauri.conf.json` → `"version"`
3. `bootready-ui/src/components/Settings.tsx` → `InfoRow label="버전"` 의 value

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

- **boot-core** (Rust): 부팅 시 시작프로그램 감시, SQLite 저장, 트레이 아이콘, Named Pipe IPC
- **bootready-ui** (Tauri v2 + React): 팝업 UI, DB 직접 읽기, show.signal 감시

## 주요 경로 (런타임)

| 파일 | 경로 |
|------|------|
| DB | `%APPDATA%\BootReady\data.db` |
| 설정 | `%APPDATA%\BootReady\config.json` |
| 시그널 | `%APPDATA%\BootReady\show.signal` |
| 아이콘 캐시 | `%APPDATA%\BootReady\icons\` |

## 브랜치 전략

- `master`: 릴리즈 브랜치
- 기능 개발: `feature/xxx` 브랜치 → master 머지

## TODO

- [ ] `get_file_icon`: Windows SHGetFileInfo 네이티브 구현 (현재 PowerShell 방식)
