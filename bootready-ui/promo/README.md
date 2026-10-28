# 홍보물 프로토타입

인스타 캐러셀(1080 x 1080, 7장)을 브라우저에서 실제로 만져 볼 수 있게 만든 프로토타입.
앱 본체와 분리되어 있고 `bootready-ui/src`는 수정하지 않는다.

## 실행

```bash
cd bootready-ui
npx vite --config promo/vite.config.ts       # http://localhost:5174
npx vite build --config promo/vite.config.ts # promo/dist (정적 서버로 열 것, file:// 불가)
```

## 무엇이 진짜인가

- 5장(설정)과 6장(팝업)은 실제 `App` / `Settings` / `TrayPopup` 컴포넌트를 iframe으로 그대로 렌더한다.
  바뀐 것은 `window.api` 대역(`mockApi.ts`) 뿐이다.
- 5장에서 URL을 추가, 수정, 삭제하면 같은 저장소(localStorage)를 거쳐 6장에서 그대로 열린다.
- 1~4장의 바탕화면, 브라우저 창, 커서와 6장의 브라우저 창은 연출이다. 로고 없이 글자만 쓰고, 주소는 예약 도메인(example.com)이다.

## 알려진 차이와 확인 필요

- 6장 팝업의 부팅 점수, 시간, 프로그램 이름은 가짜 값이다. 발행 전 실제 PC 캡처로 교체한다.
- 실제 앱은 로그온 후 10분 이내에만 팝업과 URL을 연다(`watcher.rs`). 시뮬레이션은 항상 연다.
- 7장의 "프로필 링크"가 가리킬 받는 곳(GitHub Releases 또는 스토어)이 정해지지 않았다.
