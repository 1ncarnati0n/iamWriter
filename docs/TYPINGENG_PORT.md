# typingEng → iamWriter 이식 계획

typingEng(따라치기 + 영작 첨삭 영어 학습 앱)의 기능을 **바꾸지 않고** iamWriter 안으로 옮긴다.
둘 다 Rust + iced 0.14 앱이라 이식은 대부분 파일 이동과 연결 작업이다.

- 작성: 2026-11-08
- 상태: **구현 완료(2026-11-08).** 결과와 남은 수동 점검은 10장 참고.

## 0. 요약

- **형태:** 한 창에 **집중 글쓰기 모드**(기존 iamWriter)와 **영어 공부 모드**(typingEng)를 둔다. ⌘1/⌘2와 타이틀바 오른쪽의 `집중 글쓰기 · 영어 공부` 버튼으로 바꾼다.
- **코드:** 얇은 앱 셸(`main.rs`)이 모드와 메시지를 나눠 준다. 기존 편집기는 `writer.rs`로 통째로 옮긴다. typingEng 모듈은 `practice/`로 거의 그대로 복사한다.
- **새 의존성:** 없다. 두 프로젝트의 의존성(iced 0.14, serde, serde_json)이 같다.
- **데이터:** typingEng 보관함·통계를 그대로 이어받는다. 앞으로는 `~/Library/Application Support/iamWriter/`에 저장하고, 옛 파일은 지우지 않는다.

### 결정 (2026-11-08 확정: 모두 추천안)

| 결정 | 추천 | 다른 선택 |
|---|---|---|
| 통합 형태 | **한 창, 두 모드** | 별도 창(iced 다중 창) · 별도 실행 파일(워크스페이스) |
| 데이터 위치 | **iamWriter 폴더로 옮기고 옛 파일은 읽기만** | typingEng 경로(`~/.typing_eng*.json`)를 그대로 함께 씀 |
| 독립 typingEng | **이식 후 사용 중단**(저장소는 보관) | 계속 사용. 이 경우 두 앱의 데이터가 따로 쌓인다 |

## 1. 기준선 (2026-11-08 확인)

| | iamWriter | typingEng |
|---|---|---|
| 코드 | `main.rs` 1,110줄 + `tests.rs` 282줄 | 7개 파일 2,399줄 (`main.rs` 1,126줄) |
| 의존성 | iced 0.14(advanced), rfd, serde, serde_json + 로컬 패치(iced_graphics, winit) | iced 0.14(advanced), serde, serde_json |
| 같은 버전 | iced_widget 0.14.2 · iced_graphics 0.14.0 · winit 0.30.13 · cosmic-text 0.15.0 | 같음 |
| `cargo test` | **21 통과**, 1 제외(성능 비교) | **12 통과**, 4 제외(실제 CLI·네트워크 호출) |
| `cargo clippy --all-targets` | 경고 0 | 경고 2(`youtube.rs:53` 문서 들여쓰기, `main.rs:156` 복잡한 타입) |

**지켜야 할 사용자 데이터** (`~/.typing_eng*.json`)

- 보관함
  - 영상 1개("How to Learn Rust in 2026 …"): 13세트 중 5세트 변환됨(AI 호출로 만든 것), 진행 4세트
  - AI 레슨 1개
- 통계: 누적 69줄, 최고 33 WPM, 자주 틀린 단어 13개
- 설정: 엔진 Claude, 영문 JetBrains Mono, 한글 MaruBuriOTF, 쿠키 사용

**외부 도구:** `claude`, `codex`, `yt-dlp`가 모두 PATH에 있다.

## 2. 이식 범위

### 옮길 기능 (검증 체크리스트로도 쓴다)

1. **AI 엔진:** Claude Code CLI(최신 Haiku) / Codex CLI(최신 Terra) 선택. 상태줄에 실제 모델 이름을 표시한다.
2. **AI 문장 생성:** 레벨(A1~B2) · 장르(채팅/소설) · 주제를 고르면 영어와 한국어 뜻 8~12줄을 만든다.
3. **따라치기**
   - 글자별 맞음/틀림/남음 색. 다음에 칠 글자는 밑줄
   - 채팅이면 화자 라벨을 따로 보여 준다
   - 줄 끝까지 친 뒤 Enter로 다음 줄, Tab으로 건너뛰기(기록에서 제외), Esc로 그만두기
   - WPM · 정확도 · 진행 막대. 끝낸 줄은 흐리게 보인다
   - 틀린 단어를 누적한다
4. **직접 쓰기 + 첨삭:** 한국어 뜻만 보고 영작한다. 첨삭 결과로 교정문, 고친 곳(원문 → 수정 + 이유), 총평, 모범 답안을 보여 준다.
5. **유튜브 연습**
   - yt-dlp로 자막 트랙 하나만 받는다(en-orig 우선). Chrome 쿠키를 쓰고, 429 응답이면 다시 시도한다.
   - 약 120단어씩 세트로 나누고, AI로 구두점을 복원하고 번역한다. 단어 시각에 맞춰 문장 구간을 정한다.
   - 브라우저 플레이어: 처음 한 번 클릭으로 연결한다. 문장 구간만 재생하고 멈춘다. 다시 듣기, 0.75배속, 다음 세트, 이어하기를 지원한다.
6. **보관함:** 레슨과 영상을 자동 저장한다. 열기 · 삭제 · 이어하기가 있고, 이미 받은 영상은 다시 요청하지 않는다.
7. **통계:** 누적 줄 수, 최고 WPM, 자주 틀린 단어 15개
8. **연습 글꼴:** 영문과 한글을 따로 고른다. 설치되지 않은 글꼴은 흐리게 보인다.
9. **설정:** 쿠키 사용 켜기/끄기

### 범위 밖

- 집중 글쓰기 모드와 영어 공부 모드를 섞는 새 기능. 9장 "나중에"에 따로 적었다.
- 두 모드의 글꼴 설정 합치기. 집중 글쓰기 모드는 본문 글꼴 하나, 영어 공부 모드는 영문/한글 글꼴 두 개를 그대로 쓴다.
- iamWriter 편집기 개선 로드맵. `DESIGN.md`가 그대로 유효하다.

## 3. 아키텍처

### 3.1 통합 형태: 한 창, 두 모드

```
┌ 창 (투명 타이틀바) ─────────────────────────────── [집중 글쓰기 · 영어 공부] ┐   ← 앱 셸이 겹쳐 그림
│                                                                │
│   Mode::Write    → Writer::view()     (기존 iamWriter 그대로)   │
│   Mode::Practice → Practice::view(p)  (typingEng 3화면)         │
│                    설정 → 따라치기 → 직접 쓰기                     │
└────────────────────────────────────────────────────────────────┘
```

다른 형태를 고르지 않은 이유:
- **별도 창(iced 다중 창):** 창 두 개의 포커스, 닫기 가드, 테마를 따로 맞춰야 한다. iamWriter의 투명 타이틀바와 닫기 처리를 창마다 다시 짜야 한다.
- **별도 실행 파일:** "이식"이 아니다. 테마와 팔레트 코드가 중복된다.
- **한 파일로 합치기:** 3,900줄짜리 `main.rs`가 된다.

### 3.2 모듈 구조

```
src/
├── main.rs          앱 셸: Mode, Message 라우팅, 공통 구독, 모드 전환 버튼, 창 설정   (신규 ~130줄)
├── theme.rs         Theme 5종 · Palette(+muted/ok/bad) · UI_FONT · TITLEBAR          (이동 ~90줄)
├── writer.rs        Writer: 기존 iamWriter 본체 그대로                                (이동 ~1,020줄)
├── writer/
│   └── tests.rs     기존 tests.rs 그대로                                              (이동)
└── practice/        typingEng 이식
    ├── mod.rs       Practice: typingEng main.rs 의 App (3화면 + 스타일 조각)          (~1,110줄)
    ├── claude.rs    그대로: Claude / Codex CLI 호출, Line · Review
    ├── youtube.rs   거의 그대로: crate:: → crate::practice::
    ├── player.rs    그대로: 127.0.0.1 미니 HTTP 서버
    ├── player.html  그대로: YouTube IFrame 플레이어
    ├── store.rs     저장 위치 변경 + 옛 파일 읽기
    └── typing.rs    그대로: 글자 판정 · WPM · 틀린 단어
```

- 파일 이름은 typingEng와 같게 둔다. `claude.rs`는 Codex도 다루지만 이름을 바꾸지 않는다. 비교와 추적이 쉽기 때문이다.
- `writer.rs`는 더 쪼개지 않는다. 마크다운·IME·파일 저장을 나누는 일은 이식이 끝난 뒤 따로 판단한다.
- 공용은 `theme.rs` 하나뿐이다. typingEng의 스타일 조각(`primary`, `pill`, `card`, `section` …)은 영어 공부 모드만 쓰므로 `practice/mod.rs`에 남긴다.

**파일 대응표**

| 원본 | 이식 위치 | 변경 |
|---|---|---|
| iamWriter `main.rs` | `writer.rs` + `theme.rs` + `main.rs` | 이동. 구독 2개를 앱으로 올리고 공개 범위 조정 |
| iamWriter `tests.rs` | `writer/tests.rs` | 없음 |
| typingEng `main.rs` | `practice/mod.rs` | 5장 변경 목록 참고 |
| typingEng `claude.rs` · `player.rs` · `player.html` · `typing.rs` | `practice/` | 없음 |
| typingEng `youtube.rs` | `practice/youtube.rs` | `crate::` → `crate::practice::`, 문서 주석 한 줄 |
| typingEng `store.rs` | `practice/store.rs` | 저장 경로, 옛 파일 읽기, 테스트 격리 |

### 3.3 상태와 메시지 흐름

iced의 중첩 Elm 패턴을 쓴다. 각 모드는 자기 `Message`와 `update`/`view`를 그대로 가지고, 앱 셸은 감싸서 전달만 한다.

```rust
enum Mode { Write, Practice }

struct App { mode: Mode, writer: Writer, practice: Practice }

enum Message {
    Writer(writer::Message),     // 기존 Message 그대로
    Practice(practice::Msg),     // typingEng Msg 그대로
    Switch(Mode),
}

fn update(&mut self, m: Message) -> Task<Message> {
    match m {
        Message::Writer(m) => self.writer.update(m).map(Message::Writer),
        Message::Practice(m) => self.practice.update(m).map(Message::Practice),
        Message::Switch(mode) => self.switch(mode),   // 포커스 복원 + 숨은 모드 처리 (4.1, 4.9)
    }
}

fn view(&self) -> Element<Message> {
    let p = self.writer.palette();                    // 테마는 하나. 쓰기 설정의 테마를 두 모드가 같이 쓴다
    let body = match self.mode {
        Mode::Write => self.writer.view().map(Message::Writer),
        Mode::Practice => self.practice.view(p).map(Message::Practice),
    };
    stack![body, self.mode_switch(p)].into()           // 타이틀바 오른쪽 [집중 글쓰기 · 영어 공부]
}
```

- **창 제목:** 집중 글쓰기 모드는 `파일 이름 •`, 영어 공부 모드는 `Practice::title()`이다(처음 화면은 `영어 공부`, 유튜브 따라치기는 `{영상 제목} · 세트 n/m`). 타이틀바를 숨겨 두었으므로 Dock·미션 컨트롤에서만 보인다.
- **테마:** `iced::Theme::custom`은 쓰기 설정에서 하나만 만든다.

**외부 연동 (변경 없음)**

```
Practice ── claude.rs ──(하위 프로세스, 별도 스레드)──► claude -p / codex exec   (JSON Schema 구조화 출력)
         ├─ youtube.rs ─(하위 프로세스, 별도 스레드)──► yt-dlp                   (자막 1트랙)
         ├─ player.rs ──► 127.0.0.1:<임의 포트> ◄─0.2초마다 /cmd─ 브라우저(player.html)
         └─ store.rs ───► ~/Library/Application Support/iamWriter/practice*.json
```

- 블로킹 호출은 typingEng처럼 `off_thread` + oneshot 채널 + `Task::perform`으로 처리한다.
- 결과는 `Message::Practice(..)`로 감싸 들어온다. 따라서 **모드를 바꿔도 결과가 사라지지 않는다.**

## 4. 설계 규칙 (구현 중 반드시 지킬 것)

### 4.1 숨은 모드는 부수효과를 일으키지 않는다

AI 문장이나 유튜브 세트 변환은 수 초에서 수십 초 걸린다. 그 사이 사용자가 집중 글쓰기 모드로 돌아갈 수 있다.

**확인한 위험**
- `begin()`은 `operation::focus("typing")`를 돌려준다. iced 0.14의 이 연산은 **대상이 아닌 모든 위젯의 포커스를 해제**한다(`iced_core/widget/operation/focusable.rs`). 그대로 두면 결과가 도착하는 순간 쓰기 편집기의 포커스가 빠진다.
- `begin()` 안의 `play_current()`는 플레이어 서버를 띄우고 **브라우저를 열어 영상을 재생**한다.

**규칙**
- `Practice`에 `visible: bool` 필드를 둔다. 숨어 있을 때는 상태만 바꾸고 포커스와 재생은 하지 않는다.
- 앱은 집중 글쓰기 모드로 시작하므로 `visible`의 시작값은 `false`다.
- 영어 공부 모드로 돌아오면 `resume()`이 미뤄 둔 일을 한다.

```rust
fn begin(&mut self, lines: Vec<Line>) -> Task<Msg> { /* 상태 초기화, screen = Typing */ self.resume() }

/// 보일 때만: 따라치기 화면이면 현재 문장을 재생하고 입력칸에 포커스
pub fn resume(&mut self) -> Task<Msg> {
    if !self.visible || self.screen != Screen::Typing { return Task::none(); }
    self.play_current();
    operation::focus(INPUT_ID)
}
```

나머지 `play_current()` 호출(Enter · Tab · 다시 듣기 · 배속 · 플레이어 열기)은 모두 연습 화면 조작에서만 생기므로 그대로 둔다.

### 4.2 닫기·테마 구독은 앱이 항상 가진다

- 앱은 `exit_on_close_request(false)`로 시작한다. 그래서 `window::close_requests()`를 구독하지 않는 모드에서는 **창을 닫을 수 없다.**
- 그래서 닫기 요청 구독과 `system::theme_changes()`는 앱 셸이 모드와 상관없이 항상 갖는다.
  - 닫기 요청은 `writer::Message::CloseRequested`로 넘긴다. 영어 공부 모드에서 창을 닫아도 저장 안 된 글을 확인한다.
  - 시스템 테마 변경은 `writer::Message::SystemMode`로 넘긴다.
- `Writer::subscription()`과 typingEng `subscription()`에서는 이 둘을 지운다. 같은 구독이 겹치는 것을 막기 위해서다.

### 4.3 `Subscription::map`의 클로저는 아무것도 캡처하지 않는다

iced 0.14는 `map`에 넘긴 클로저의 크기가 0인지 **컴파일 단계에서** 검사한다(`const { check_zero_sized::<F>() }`).
- `Message::Writer`처럼 생성자를 그대로 넘기거나, `|id| Message::Writer(writer::Message::CloseRequested(id))`처럼 바깥 값을 쓰지 않는 클로저만 쓴다.
- 모드 값 같은 상태가 필요하면 구독을 만들 때 `match`로 갈라야 한다. 클로저 안에서 상태를 읽으면 안 된다.

### 4.4 키보드 구독은 모드마다 따로

```rust
fn subscription(&self) -> Subscription<Message> {
    let mode = match self.mode {
        Mode::Write => self.writer.subscription().map(Message::Writer),
        Mode::Practice => self.practice.subscription().map(Message::Practice),
    };
    Subscription::batch([mode, switch_keys(), close_requests(), theme_changes()])
}
```

| 키 | 집중 글쓰기 모드 | 영어 공부 모드 | 처리 위치 |
|---|---|---|---|
| ⌘1 / ⌘2 | 쓰기 / 연습으로 전환 | 같음 | 앱 |
| 창 닫기 | 저장 안 된 글 확인 | **같음(쓰기 문서 기준)** | 앱 → 쓰기 |
| Esc | 검색·설정 닫고 본문으로 | 오류 닫기, 따라치기 중이면 처음 화면 | 각 모드 |
| Tab | 위젯이 쓰지 않으면 포커스 이동 | 따라치기 중 건너뛰기 | 각 모드 |
| Enter | 검색창의 다음 결과 | 입력칸에서 다음 줄, 빈 칸이면 다시 듣기 | 각 모드 |
| ⌘S ⌘O ⌘N ⌘F ⌘, ⌘Z ⌘B ⌘I ⇧⌘F ⇧⌘D | 쓰기 기능 | **동작 안 함** | 쓰기 |
| ⌘C ⌘X ⌘V ⌘A | 편집기 | 입력칸·작문칸 | 위젯 |

- 두 모드의 구독을 함께 켜 두면 Tab 한 번에 포커스 이동과 건너뛰기가 **동시에** 일어난다. 그래서 반드시 모드별로 하나만 켠다.
- 쓰기 구독은 ⌘ 조합을 `Shortcut("1")`처럼 보내지만, 쓰기 쪽이 처리하지 않는 키라 문제없다.
- 테마 순환(⇧⌘D)은 지금처럼 집중 글쓰기 모드에만 둔다. 영어 공부 모드에서 테마를 바꾸려면 집중 글쓰기 모드의 설정을 쓴다.

### 4.5 팔레트: `dim`의 뜻이 두 가지다

| 이름 | iamWriter | typingEng |
|---|---|---|
| `dim` | 포커스 모드에서 흐려진 글자. 아주 옅음(다크 85 회색) | 보조 설명 글자. 읽혀야 함(다크 120 회색) |

- 공용 `Palette`에 `muted`(읽히는 보조 글자색)를 새로 두고, typingEng 코드의 `p.dim`은 모두 `p.muted`로 바꾼다.
- `ok`·`bad`(맞음/틀림)는 테마 5종 모두에 정의한다. `hair()`(옅은 구분선)는 공용 `Palette`로 옮긴다.

시작값(대비 비율은 계산 결과):

| 테마 | `muted` | `ok` | `bad` |
|---|---|---|---|
| 라이트 | `g(140)` · 3.1:1 | `(30,135,75)` · 4.2:1 | `(205,50,60)` · 4.8:1 |
| 세피아 | `(140,115,90)` · 3.8:1 | 라이트와 같음 · 3.9:1 | 라이트와 같음 · 4.3:1 |
| 다크 | `g(120)` · 3.9:1 | `(120,200,140)` · 8.6:1 | `(240,110,120)` · 6.0:1 |
| 나이트 | `(110,118,135)` · 4.3:1 | 다크와 같음 · 9.7:1 | 다크와 같음 · 6.7:1 |

**테스트:** 테마 5종 모두에서 `muted`·`ok`·`bad`가 배경 대비 **3:1 이상**, `fg`가 **4.5:1 이상**인지 확인한다.
- `dim`은 일부러 옅게 둔 색이라 검사하지 않는다.
- 숫자는 이 테스트를 통과하는 범위에서 바꿔도 된다.

### 4.6 모듈 경로

- `practice/` 안의 모든 `crate::` 경로는 `crate::practice::`로 바꾼다. `youtube.rs`와 `store.rs`의 `crate::claude::…`, `crate::youtube::…`가 대상이다.
- `super::`로 바꾸면 **안 된다.** `youtube.rs`의 `mod tests` 안에서 `super::`는 `practice`가 아니라 `youtube`를 가리킨다. 그래서 `live_pipeline`의 `crate::claude::…`를 `super::claude::…`로 바꾸면 경로가 풀리지 않는다.
  - `#[ignore]` 테스트도 컴파일은 되므로, 그렇게 바꾸면 3단계 빌드가 실패한다.
  - 한 가지 규칙(`crate::practice::`)만 쓰는 쪽이 안전하다.
- 앱 셸이 크레이트 루트가 되므로 영어 공부 모드 쪽 공용 항목은 `crate::theme::{Palette, UI_FONT, TITLEBAR}`로 가져온다.

### 4.7 데이터 위치와 이전

| 파일 | 새 위치 | 처음 한 번 읽는 옛 위치 |
|---|---|---|
| 통계·연습 설정 | `~/Library/Application Support/iamWriter/practice.json` | `~/.typing_eng.json` |
| 보관함 | `~/Library/Application Support/iamWriter/practice-library.json` | `~/.typing_eng_library.json` |

- **불러오기:** 새 파일이 있으면 그것을 읽는다. 없을 때만 옛 파일을 읽는다. 형식(JSON 스키마)은 그대로다.
- **저장:** 새 위치에만 쓴다. 폴더를 먼저 만들고(`create_dir_all`), typingEng처럼 임시 파일에 쓴 뒤 `rename`한다.
- **옛 파일은 지우거나 고치지 않는다.** 백업 역할을 한다.
- 독립 typingEng를 계속 쓰면 그 앱은 옛 파일에, iamWriter는 새 파일에 따로 기록한다. 결정 표에서 "사용 중단"을 추천하는 이유다.

### 4.8 테스트는 실제 데이터를 절대 건드리지 않는다

보관함의 변환된 세트 5개는 실제 AI 호출로 만든 것이다. 테스트가 덮어쓰면 다시 만들 수 없다.
- `store.rs`의 저장 폴더를 `cfg!(test)`일 때 프로세스별 임시 폴더로 고정한다. 테스트 빌드인지는 컴파일할 때 정해지므로, 어떤 테스트가 `Practice`를 만들고 `save()`를 불러도 실제 경로에는 쓰지 않는다.
- 이전 로직은 경로를 인자로 받는 순수 함수(`load_first(&[새 경로, 옛 경로])`)로 만들고 임시 폴더에서 테스트한다.
- 집중 글쓰기 모드의 `settings_path()`도 같은 방식으로 막는다. 지금은 설정을 바꾸는 테스트가 없어서 문제가 없을 뿐, 잠재 위험이다.

### 4.9 모드를 바꾸면 위젯 상태가 초기화된다

- 루트 아래 위젯 종류가 바뀌므로 iced가 위젯 트리를 새로 만든다. **포커스, 입력칸 커서 위치, 스크롤 위치가 초기화된다.**
- 글 내용과 커서는 `text_editor::Content`(앱 상태)에 있어서 유지된다.
  - 편집기 스크롤 위치도 `Content` 안에 있을 것으로 보지만 확인하지 않았다. 7.3에서 직접 점검한다.
- 그래서 `switch()`는 상태를 직접 되돌린다.
  - 집중 글쓰기 모드: `focus("editor")`와 상태줄 표시(`writer.show_chrome()`)를 함께 돌려준다.
    - 영어 공부 모드에 있는 동안 집중 글쓰기 모드의 `now`와 프레임 구독이 멈춘다.
    - 그래서 흐려지던 중에 전환했다면 상태줄이 반쯤 흐린 채 남는다. 마우스를 움직이기 전까지 그대로다.
  - 영어 공부 모드: `practice.resume()` (따라치기 화면일 때만 포커스와 재생)
- 영어 공부 모드의 처음 화면(설정 화면) 스크롤이 맨 위로 돌아가는 것은 받아들인다.

## 5. 의도적 변경 목록

여기 없는 것은 원본과 **바이트 단위로 같아야** 한다. 리뷰할 때 `diff`로 확인한다.

**`practice/mod.rs` (typingEng `main.rs`에서)**

1. `fn main()`과 창 설정을 지운다. 창 크기는 iamWriter 값(1000×760)을 쓴다.
2. 이름을 바꾼다: `App` → `Practice`, `App::new` → `Practice::new`. 시스템 테마를 묻는 `Task`는 앱이 하므로 돌려주지 않는다.
3. 자체 `Palette`와 `Palette::new`를 지우고 `crate::theme::Palette`를 쓴다. `p.dim`은 모두 `p.muted`로 바꾼다.
4. `system_dark` 필드, `Msg::SystemMode`, `palette()`, `theme()`를 지운다. `view(&self)`는 `view(&self, p: Palette)`가 된다.
5. `subscription()`에서 `system::theme_changes()`를 지운다. Esc·Tab 처리는 그대로 둔다.
6. `visible` 필드와 `resume()`를 추가하고, `begin()`의 끝을 `resume()` 호출로 바꾼다(4.1).
7. 표시 문구: 타이틀바와 처음 화면 제목 "typingEng"를 "영어 공부"로 바꾼다. 부제 "따라 치고, 직접 써보며 익히는 영어"는 유지한다.
8. `TITLEBAR`·`UI_FONT`는 `crate::theme`에서 가져온다.
9. clippy: `fonts` 필드의 튜플 타입을 타입 별칭으로 바꾼다(`type_complexity`).
10. `fn title(&self) -> String`을 추가한다. 지금 `view`에 있는 제목 계산을 빼낸 것으로, 창 제목에 쓴다.

**`practice/store.rs`**

11. 저장 위치, 옛 파일 읽기, 폴더 생성, 테스트 격리(4.7, 4.8)

**`practice/youtube.rs`**

12. `crate::` → `crate::practice::` (4.6. `store.rs`도 같음)
13. 53줄 문서 주석 앞에 빈 `///` 줄을 넣는다(clippy `doc_lazy_continuation`).

**집중 글쓰기 모드 (`writer.rs`)**

14. `Theme`·`Palette`·`UI_FONT`·`TITLEBAR`를 `theme.rs`로 옮긴다. `Palette`에 `muted`·`ok`·`bad`·`hair()`를 추가한다.
15. `subscription()`에서 닫기 요청과 시스템 테마 구독을 뺀다(4.2).
16. 앱 셸이 쓰도록 공개 범위를 `pub(crate)`로 연다. 두 메서드를 추가한다.
    - `chrome_alpha()`: 모드 전환 버튼이 타이핑 중 함께 흐려지게 한다.
    - `show_chrome()`: `now`를 갱신하고 `chrome.go_mut(true, now)`를 부른다. 모드 전환 시 쓴다(4.9).
17. `settings_path()`의 테스트 격리(4.8)

**선택:** `player.html`의 `<title>typingEng 플레이어</title>`를 바꿀지는 결정이 필요 없을 만큼 사소하다. 기본은 그대로 둔다.

## 6. 작업 단계

각 단계가 끝날 때 `cargo test`와 `cargo clippy --all-targets`가 통과해야 한다. 커밋은 단계마다 사용자가 직접 한다.

| 단계 | 내용 | 완료 조건 |
|---|---|---|
| **1. 집중 글쓰기 모드 분리** | `main.rs` → `writer.rs` + `theme.rs`, `tests.rs` → `writer/tests.rs`. `main()`은 지금처럼 `Writer`만 실행한다. **동작 변화 없음** | 테스트 21 통과 / 1 제외, clippy 0, 실행 화면 동일 |
| **2. 앱 셸 (쓰기만)** | `App { mode: Write, writer }`, `Message::Writer`. 닫기·테마 구독을 앱으로 옮긴다 | 21 / 1. 저장 안 된 글이 있을 때 창을 닫으면 저장을 묻는다. 시스템 다크 모드 전환을 따라간다 |
| **3. 로직 모듈 복사** | `practice/`에 `claude`·`youtube`·`player`(+html)·`store`·`typing`을 넣는다. 경로 수정, 저장 위치·이전·테스트 격리 | `cargo test practice::` **11 통과 / 4 제외** + 이전 테스트 통과 |
| **4. 연습 화면** | typingEng `main.rs` → `practice/mod.rs` (5장 목록대로). `Mode::Practice`를 연결한다(임시로 ⌘2만) | **33 통과 / 5 제외** + 새 테스트, clippy 0. 기존 데이터가 그대로 보인다(7.2) |
| **5. 모드 전환 UX** | 타이틀바 `[집중 글쓰기 · 영어 공부]`, ⌘1/⌘2, 포커스 복원, `visible`/`resume()` | 7.3 수동 점검, 숨은 모드 테스트 통과 |
| **6. 검증과 문서** | 7장 전체 점검. README(영어 공부 모드, 준비물 `claude`/`codex`/`yt-dlp`), DESIGN, IMPLEMENTATION 갱신 | 체크리스트 완료, 문서 반영 |

- 3단계에서는 화면이 아직 모듈을 쓰지 않으므로 `dead_code` 경고가 잠시 생길 수 있다. 4단계에서 사라진다. 3단계에서만 `practice` 모듈에 `#[allow(dead_code)]`를 붙이고 4단계에서 지운다.
- 2단계와 4단계를 나눈 이유: 구독 위치를 옮기는 일(닫기 가드가 깨질 수 있음)과 화면을 옮기는 일을 따로 검증하기 위해서다.

## 7. 검증

### 7.1 자동

- `cargo test`: 기존 **33 통과 / 5 제외**(21+12, 1+4)에 새 테스트를 더한다. 새 테스트:
  - 테마 5종의 `muted`·`ok`·`bad`·`fg` 대비(4.5)
  - 저장소 이전: 새 파일이 없으면 옛 파일을 읽고, 새 위치에만 쓰고, 옛 파일은 그대로인지(임시 폴더에서)
  - 숨은 모드: `begin()` 후에도 플레이어가 시작되지 않는지(`player.is_none()`)
    - 조건: `visible = false`이고 `cur_video = Some(..)`이다.
    - `cur_video`가 `None`이면 `play_current()`가 처음부터 아무것도 하지 않으므로, 확인 대상과 무관하게 테스트가 항상 통과한다.
    - `visible = true` 쪽은 단위 테스트로 확인하지 않는다. 실제로 플레이어 서버를 띄우고 브라우저를 열기 때문이다. 7.3에서 직접 점검한다.
  - 앱 라우팅: `Switch`가 모드를 바꾸고 `practice.visible`과 맞는지
    - 이 테스트는 `App::boot()`나 `Writer::boot()`를 부르지 않는다. 테스트 실행 중에는 `args().nth(1)`이 테스트 이름이라 파일 경로로 열린다.
    - 대신 `Writer::with_path(None)`과 `Practice::new()`로 `App`을 직접 만든다.
- `cargo clippy --all-targets`: 경고 0. 외부 `block` 크레이트 경고는 기존과 같다.
- (선택, 사용량 소모) `cargo test -- --ignored`: 실제 Claude/Codex/yt-dlp 호출 4개와 성능 비교 1개

### 7.2 기존 데이터 확인 (영어 공부 모드 처음 화면)

- [ ] 보관함에 "How to Learn Rust in 2026 …"이 **세트 4/13 완료**로 보이고, 이어하기를 누르면 변환된 5번째 세트가 AI 호출 없이 바로 열린다
- [ ] AI 레슨 1개가 보인다
- [ ] 상태줄에 누적 69줄 · 최고 33 WPM, 자주 틀린 단어 13개
- [ ] 엔진 Claude, 영문 JetBrains Mono, 한글 마루 부리, 쿠키 사용이 선택돼 있다
- [ ] 무언가를 저장한 뒤 `practice*.json`이 생기고, `~/.typing_eng*.json`의 수정 시각이 그대로다

### 7.3 수동 점검

**영어 공부 모드**: 2장의 기능 1~9를 각각 한 번씩 확인한다.

**모드 전환**
- [ ] ⌘1/⌘2와 타이틀바 버튼으로 전환된다. 전환 직후 쓰기 편집기 / 따라치기 입력칸에 바로 입력된다
- [ ] 긴 글을 아래로 스크롤한 뒤 연습 → 쓰기로 돌아와도 스크롤 위치가 유지된다(4.9. 안 되면 위치 저장을 추가한다)
- [ ] 타이핑으로 상태줄이 흐려지던 중 전환했다가 돌아오면 상태줄이 다시 보인다
- [ ] 전환 버튼이 없는 타이틀바 영역은 여전히 창 끌기·더블클릭 확대가 된다
- [ ] 좁은 창에서 긴 영상 제목과 전환 버튼이 겹치지 않는다
- [ ] AI 문장 생성 중 집중 글쓰기 모드로 전환한 뒤 계속 타이핑한다. 생성이 끝나도 **편집기 포커스가 빠지지 않고 브라우저가 열리지 않는다**. 영어 공부 모드로 돌아가면 그때 시작된다
- [ ] 유튜브 세트 변환 중에도 같다(재생은 돌아온 뒤에)
- [ ] 영어 공부 모드의 Tab은 건너뛰기만 하고, 집중 글쓰기 모드의 Tab은 포커스 이동만 한다
- [ ] 영어 공부 모드에서 ⌘S·⌘Z·⌘B가 쓰기 문서를 바꾸지 않는다

**닫기**
- [ ] 쓰기 문서가 저장 안 된 상태에서 **영어 공부 모드일 때** 창을 닫으면 저장을 묻는다. 취소하면 창이 남는다

**테마**
- [ ] 테마 5종에서 연습 화면의 맞음/틀림/보조 글자가 읽힌다. 시스템 다크 모드 전환도 따라간다

**집중 글쓰기 모드 회귀**
- [ ] 편집, 저장, 검색, 설정, 포커스 모드, 한글 조합 표시(IME), ⌘B/⌘I, 실행 취소
- [ ] `IMPLEMENTATION.md`의 남은 수동 점검 항목은 그대로 유효하다

## 8. 위험과 제한

| 위험 | 대응 |
|---|---|
| 숨은 모드에서 포커스를 뺏거나 영상을 재생함 | 4.1 규칙 + 테스트 + 7.3 점검 |
| 영어 공부 모드에서 창이 닫히지 않음 | 4.2. 2단계에서 먼저 검증 |
| 테스트가 실제 보관함을 덮어씀 | 4.8 `cfg!(test)` 격리. 4단계 전에 이전 테스트로 확인 |
| Tab 등 키가 두 모드에서 동시에 동작 | 4.4 모드별 구독 |
| 연습 화면의 보조 글자가 너무 옅음 | 4.5 `muted` + 대비 테스트 |
| 독립 typingEng와 데이터가 갈라짐 | 결정 표. 옛 파일은 지우지 않음 |

**typingEng에서 그대로 넘어오는 제한**
- `.app`으로 묶어 Finder에서 실행하면 PATH에 `claude`/`codex`/`yt-dlp`가 없을 수 있다. 지금처럼 터미널에서 실행하는 것을 전제로 한다.
- 영상 주인이 임베드를 막은 영상은 플레이어에서 재생되지 않는다.
- 직접 쓰기 칸에는 실행 취소가 없다(iced 기본 편집기).

**새로 생기는 영향**
- iamWriter의 winit 한글 입력 패치와 iced_graphics 기울임 패치가 영어 공부 모드에도 적용된다.
  - 한글 입력 패치: 연습 입력칸도 이 경로를 쓴다. 한글 첫 키 문제를 덜어 주는 쪽이다.
  - 기울임 패치: typingEng는 `Oblique`를 쓰지 않으므로 영향이 없다.
- 앱을 켤 때 연습 데이터(약 40KB)와 연습 글꼴 목록을 함께 읽는다. 측정해 보고 느리면 영어 공부 모드를 처음 열 때 읽도록 미룬다.

## 9. 나중에 (이식 후, 필요해지면)

이식 범위 밖이다. 설계는 하지 않고 후보만 적는다.

- **지금 문서로 따라치기:** 열린 글을 AI로 문장 단위로 나누고 뜻을 붙여 레슨으로 만든다. typingEng 아이디어 목록의 "내 txt 파일 불러오기"와 같은 기능이다.
- **첨삭 결과를 새 문서로:** 교정문, 고친 곳, 총평을 마크다운으로 만들어 집중 글쓰기 모드에서 연다.
- **직접 쓰기 칸에 쓰기 편집기 재사용:** 마크다운 강조, 한글 조합 표시, 실행 취소를 얻는다.
- **연습 글꼴 선택에 `faces_of` 적용:** Regular 굵기가 없는 글꼴이 다른 글꼴로 바뀌어 보이는 문제(KoPub 사례)를 영어 공부 모드에서도 막는다.
- **숨은 모드에서 끝난 작업 알림:** 집중 글쓰기 모드 상태줄에 "연습 문장 준비됨"을 띄운다.
- **자주 틀린 단어를 다음 문장 생성에 반영, 간격 반복 복습:** typingEng 아이디어 목록에 있던 항목이다.

## 10. 구현 결과 (2026-11-08)

모드 이름은 **집중 글쓰기**(⌘1)와 **영어 공부**(⌘2)로 정했다. 6단계를 순서대로 진행했고, 단계마다 테스트와 clippy가 통과했다.

| 단계 | 결과 |
|---|---|
| 1. 집중 글쓰기 분리 | `main.rs` → `theme.rs` + `writer.rs` + `writer/tests.rs`. 테스트 21 / 1 그대로 |
| 2. 앱 셸 | 창 닫기·시스템 테마 구독을 앱 셸로 옮김. 21 / 1 |
| 3. 로직 모듈 | `practice::` 13 통과 / 4 제외 (원본 11 + 저장소 테스트 2) |
| 4~5. 화면과 모드 전환 | 전체 **39 통과 / 5 제외** (기존 33 + 새 6), clippy 경고 0, 릴리스 빌드 성공 |

**원본과의 차이 확인 (`diff`)**
- `claude.rs` · `player.rs` · `player.html` · `typing.rs`는 원본과 바이트 단위로 같다.
- `youtube.rs`는 경로 2곳과 문서 주석 한 줄만 다르다.
- `mod.rs`는 5장 목록 그대로다. `p.dim` → `p.muted`는 25곳이다.

### 계획에 없던 변경

1. **⌘ 조합의 글자 입력 막기 (`main.rs`의 `StripCommandText`)**
   - macOS는 ⌘ 조합 키에도 글자를 실어 보낸다. iced의 `text_input`은 ⌘C·X·V·A 외에는 그 글자를 그대로 입력한다(`iced_widget/text_input.rs`).
   - 그래서 ⌘1/⌘2를 구독으로만 처리하면 따라치기 입력칸에 "1"·"2"가 들어가 정확도 기록이 틀어진다.
   - 앱 전체를 감싸는 투명 래퍼가 ⌘가 눌린 키 이벤트에서 **글자만** 지운다. 키 자체는 그대로라서 위젯 단축키와 앱 단축키는 영향이 없다.
   - 집중 글쓰기의 검색칸·글꼴 검색칸에 원래 있던 같은 문제(예: 검색칸에서 ⌘F를 누르면 "f"가 붙음)도 함께 막힌다. 다만 이 문제는 코드로 확인했을 뿐, 실제 입력으로 재현하지는 않았다.
2. **설정 카드 단축키 안내에 `⌘1 집중 글쓰기 · ⌘2 영어 공부` 한 줄 추가**
3. **`Writer::with_path`를 `pub(crate)`로 연 것** (앱 셸 테스트용)

### 검증

- **새 테스트 6개**
  - 테마 5종의 대비
  - 저장소: 옛 파일 읽기, 새 위치에만 쓰기, 새 파일이 깨졌을 때 옛 파일로 돌아가기
  - 테스트 빌드가 실제 경로를 쓰지 않는지
  - 숨은 모드에서 플레이어가 시작되지 않는지(`cur_video`가 있는 상태)
  - 모드 전환과 `visible`이 맞는지
  - ⌘1/⌘2 인식과 ⌘ 글자 지우기
- **화면 없이 그린 이미지(`iced_test`, 확인 후 테스트와 의존성은 지움)**
  - 집중 글쓰기: 타이틀바 오른쪽에 전환 버튼이 보인다.
  - 영어 공부 처음 화면: 사용자의 typingEng 데이터 **복사본**을 읽어 7.2 항목이 모두 그대로 보였다.
    - 보관함: Rust 영상 "세트 4/13 완료 · 이어하기", AI 레슨 1개(11줄)
    - 기록: 자주 틀린 단어 13개, 누적 69줄 · 최고 33 WPM
    - 설정: Claude, JetBrains Mono / 마루 부리, 쿠키 사용
  - 따라치기 화면: 다크·세피아 테마 모두에서 글자별 맞음/틀림/다음 글자 표시가 정상이었다.
- **실제 데이터:** `~/.typing_eng*.json`의 수정 시각이 그대로다. 앱을 켜기만 해서는 아무 파일도 쓰지 않는다.
- **실제 앱:** 릴리스 빌드를 실행해 6초 동안 문제가 없었다. 화면 녹화 권한이 없어 실제 창으로 조작하지는 못했다.

### 남은 수동 점검

실제 창에서 해야 하는 7.3 항목이 남아 있다. 특히 다음을 먼저 확인한다.
- [ ] ⌘1/⌘2를 누른 뒤 따라치기 입력칸과 검색칸에 숫자·글자가 들어가지 않는지
- [ ] AI 문장 생성이나 유튜브 세트 변환 중에 집중 글쓰기로 전환한 뒤 계속 타이핑할 때 편집기 포커스가 유지되고 브라우저가 열리지 않는지. 영어 공부로 돌아오면 그때 시작되는지
- [ ] 영어 공부 모드에서 창을 닫을 때 저장 안 된 글을 묻는지
- [ ] 전환 버튼 밖의 타이틀바로 창 끌기와 더블클릭 확대가 되는지
- [ ] 긴 글을 스크롤한 뒤 모드를 오갔을 때 편집기 스크롤 위치가 유지되는지
- [ ] 처음 저장 뒤 `practice.json` / `practice-library.json`이 생기고, 옛 파일은 그대로인지

