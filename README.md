# iamWriter

글쓰기에만 집중하기 위한 미니멀 마크다운 에디터. Rust + [iced](https://iced.rs) 로 만든 macOS 네이티브 앱.

## 기능

- **포커스 모드** — 지금 쓰는 문단만 또렷하게, 나머지는 흐리게
- **마크다운 강조** — 제목·인용·목록 기호 색 구분, `**굵게**` `*기울임*` 은 실제 Bold·Italic 글꼴로
- **글꼴 선택** — 글쓰기용 추천 글꼴(한글 고정폭 · 한글 명조 · 한글 고딕 · 고정폭 · 세리프 · 산세리프) 중 설치된 것만 한국어 이름으로 보여 주고(D2Coding, Monoplex KR, 마루 부리 등), 한글 포함 글꼴엔 `가` 표시. 검색하면 설치된 모든 글꼴에서 찾음
- **한글 대응** — UI 는 Apple SD Gothic Neo 로 통일, Regular 굵기가 없는 글꼴(예: KoPub)도 실제 있는 굵기를 골라 다른 글꼴로 바뀌지 않음, 한글이 없는 글꼴을 고르면 안내, 조합 중인 한글을 본문과 같은 글꼴·같은 줄 위치에 표시(iced 기본 동작은 줄 아래로 처짐)
- **테마** — 자동(시스템 따라감) · 라이트 · 세피아 · 다크 · 나이트
- **방해 없는 화면** — 타이핑하면 상태 표시줄이 사라지고, 마우스를 움직이면 다시 나타남
- 단어 수 · 글자 수 · 읽는 시간 표시
- 실행 취소/다시 실행, 설정 자동 저장(`~/Library/Application Support/iamWriter/settings.json`), 저장 안 된 글 닫기 보호

## 실행

```bash
cargo run --release            # 빈 문서
cargo run --release -- 노트.md  # 파일 열기
```

Rust 1.88+ 와 macOS 가 필요합니다(타이틀바·설정 경로가 macOS 기준).

## 단축키

| 키 | 동작 |
|---|---|
| ⌘O | 열기 |
| ⌘S / ⇧⌘S | 저장 / 다른 이름으로 저장 |
| ⌘Z / ⇧⌘Z | 실행 취소 / 다시 실행 |
| ⌘, | 설정 (Esc 로 닫기) |
| ⇧⌘F | 포커스 모드 |
| ⇧⌘D | 테마 순환 |

## 개발

```bash
cargo test                       # 단위 테스트
cargo install cargo-watch && cargo watch -x run   # 저장하면 자동 재실행
```

### 구조

코드는 `src/main.rs` 한 파일에 있습니다.

| 부분 | 내용 |
|---|---|
| `Theme`, `Palette`, `Settings` | 테마 5종과 설정. 설정은 JSON 파일로 저장 |
| `inline_spans`, `line_prefix`, `para_lines` | 마크다운 기호 분석과 포커스 모드의 문단 범위 계산(단위 테스트 있음) |
| `Markdown` | `text_editor` 하이라이터: 기호 색, 굵게·기울임 글꼴, 포커스 모드 흐림 |
| `Writer` | iced 앱 본체: 파일 열기·저장, 실행 취소, 단축키, 화면 구성, 설정 카드 |
| `ImeFix` | 편집기 래퍼: 한글 조합 중 글자(preedit)를 본문 글꼴·같은 기준선에 직접 그림 |

`archive/egui_main.rs` 는 iced 로 바꾸기 전의 egui 버전입니다(빌드에 포함되지 않음).

### 알려진 제한

iced `text_editor` 의 한계로 다음 기능은 아직 없습니다.
- 행잉 제목 기호
- 타자기 모드
- 2px 커서
- 코드 배경색

해결 계획과 전체 디자인 로드맵은 [docs/DESIGN.md](docs/DESIGN.md) 참고.
