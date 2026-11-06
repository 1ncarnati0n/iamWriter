// iamWriter — 방해 없는 마크다운 글쓰기 도구 (iced)
// 단축키: ⌘O 열기 · ⌘S 저장 · ⇧⌘S 다른 이름으로 · ⌘, 설정 · ⌘Z / ⇧⌘Z 실행 취소/다시
//         ⇧⌘F 포커스 · ⇧⌘D 테마 순환
use iced::{
    advanced::graphics::text::font_system,
    event, font, keyboard, mouse, system, window,
    time::{Duration, Instant},
    widget::{
        button, column, container, mouse_area, opaque, row, scrollable, slider, space, stack, text,
        text_editor, text_input, toggler,
    },
    Animation, Border, Color, Element, Event, Font, Length, Padding, Subscription, Task,
};
use serde::{Deserialize, Serialize};
use std::{ops::Range, path::PathBuf};

const TITLEBAR: f32 = 30.0; // macOS 신호등 버튼 높이만큼
/// 모든 Mac 에 있는 한글 글꼴. UI 기본 글꼴로 쓰면 한글/영문 기준선과 높이가 섞이지 않는다.
const UI_FONT: Font = Font::with_name("Apple SD Gothic Neo");

/// 글쓰기용으로 고른 글꼴. 설치된 것만 목록에 뜬다. 검색하면 설치된 모든 글꼴에서 찾는다.
const CURATED: [(&str, &[&str]); 6] = [
    ("한글 고정폭", &["D2Coding", "D2Coding ligature", "Monoplex KR", "Monoplex KR Wide", "Sarasa Mono K", "NanumGothicCoding"]),
    ("한글 명조", &[
        "MaruBuri", "MaruBuriOTF", "Nanum Myeongjo", "Noto Serif KR", "KoPubWorldBatang_Pro", "RIDIBatang", "Gowun Batang", "AppleMyungjo",
    ]),
    ("한글 고딕", &[
        "Pretendard", "Apple SD Gothic Neo", "Noto Sans KR", "Spoqa Han Sans Neo", "Nanum Gothic", "KoPubWorldDotum_Pro",
    ]),
    ("고정폭", &[
        "iA Writer Mono S", "iA Writer Duo S", "SF Mono", "Menlo", "IBM Plex Mono", "JetBrains Mono",
        "Source Code Pro", "Fira Code", "PT Mono", "Courier Prime",
    ]),
    ("세리프", &["iA Writer Quattro S", "New York", "Charter", "Iowan Old Style", "Georgia", "Palatino", "Baskerville", "Hoefler Text"]),
    ("산세리프", &["Inter", "IBM Plex Sans", "Helvetica Neue", "Avenir Next"]),
];

fn main() -> iced::Result {
    iced::application(Writer::boot, Writer::update, Writer::view)
        .title(Writer::title)
        .theme(Writer::theme)
        .default_font(UI_FONT)
        .subscription(Writer::subscription)
        .exit_on_close_request(false) // 닫기 가드를 직접 처리
        .window(window::Settings {
            size: iced::Size::new(1000.0, 760.0),
            #[cfg(target_os = "macos")]
            platform_specific: window::settings::PlatformSpecific {
                title_hidden: true,
                titlebar_transparent: true,
                fullsize_content_view: true, // 본문 배경이 타이틀바 뒤까지
            },
            ..Default::default()
        })
        .run()
}

// ───────────────────────── 설정 / 테마 ─────────────────────────

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
enum Theme { Auto, Light, Sepia, Dark, Night }

#[derive(Clone, Copy, PartialEq, Debug)]
struct Palette { bg: Color, panel: Color, fg: Color, strong: Color, dim: Color, accent: Color, code: Color }

impl Theme {
    const ALL: [(Theme, &'static str); 5] = [
        (Theme::Auto, "자동"), (Theme::Light, "라이트"), (Theme::Sepia, "세피아"), (Theme::Dark, "다크"), (Theme::Night, "나이트"),
    ];

    fn resolve(self, system_dark: bool) -> Theme {
        match self {
            Theme::Auto if system_dark => Theme::Dark,
            Theme::Auto => Theme::Light,
            t => t,
        }
    }

    fn next(self) -> Self {
        let i = Self::ALL.iter().position(|(t, _)| *t == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()].0
    }

    fn palette(self) -> Palette {
        let c = Color::from_rgb8;
        let g = |v| Color::from_rgb8(v, v, v);
        match self {
            Theme::Light | Theme::Auto => Palette { bg: c(247, 247, 245), panel: c(255, 255, 255), fg: g(45), strong: g(0), dim: g(185), accent: c(20, 110, 230), code: c(170, 60, 120) },
            Theme::Sepia => Palette { bg: c(244, 236, 216), panel: c(250, 244, 230), fg: c(91, 70, 54), strong: c(45, 30, 20), dim: c(195, 175, 145), accent: c(185, 95, 40), code: c(140, 90, 150) },
            Theme::Dark => Palette { bg: c(26, 26, 28), panel: c(38, 38, 42), fg: g(215), strong: g(250), dim: g(85), accent: c(90, 160, 255), code: c(220, 140, 190) },
            Theme::Night => Palette { bg: c(10, 12, 18), panel: c(22, 26, 36), fg: c(165, 175, 190), strong: c(230, 235, 245), dim: c(55, 60, 75), accent: c(110, 200, 165), code: c(200, 160, 230) },
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
struct Settings {
    font: Option<String>, // 글꼴 패밀리 이름, None = Menlo
    size: f32,
    line: f32,
    measure: u32, // 본문 폭(글자 수)
    theme: Theme,
    focus: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { font: None, size: 18.0, line: 1.7, measure: 64, theme: Theme::Auto, focus: false }
    }
}

fn settings_path() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join("Library/Application Support/iamWriter/settings.json")
}

fn load_settings() -> Settings {
    std::fs::read(settings_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn store_settings(s: &Settings) {
    let p = settings_path();
    let _ = std::fs::create_dir_all(p.parent().unwrap());
    let _ = std::fs::write(p, serde_json::to_vec_pretty(s).unwrap()); // 설정 저장 실패는 글을 잃지 않으므로 무시
}

/// iced(cosmic-text)가 실제로 쓸 수 있는 설치 글꼴 패밀리 이름들.
/// Font 가 &'static str 이름만 받으므로 시작할 때 한 번 'static 으로 만든다.
fn font_families() -> Vec<&'static str> {
    let mut fs = font_system().write().expect("font system");
    let mut v: Vec<String> = fs.raw().db().faces()
        .filter_map(|f| f.families.first().map(|(n, _)| n.clone()))
        .filter(|n| !n.starts_with('.'))
        .collect();
    v.sort_by_key(|n| n.to_lowercase());
    v.dedup();
    v.into_iter().map(|n| &*Box::leak(n.into_boxed_str())).collect()
}

/// 이 패밀리가 한글('가')을 직접 갖고 있는지. 실제로 셰이핑해 보고 대체 글꼴이 쓰였는지 본다.
fn has_hangul(family: &'static str) -> bool {
    use iced::advanced::graphics::text::cosmic_text::{Attrs, Buffer, Family, Metrics, Shaping, Weight};
    let weight = Weight(faces_of(family).regular.weight as u16 * 100 + 100);
    let mut fs = font_system().write().expect("font system");
    let fs = fs.raw();
    let mut b = Buffer::new(fs, Metrics::new(16.0, 20.0));
    b.set_text(fs, "가", &Attrs::new().family(Family::Name(family)).weight(weight), Shaping::Advanced, None);
    b.shape_until_scroll(fs, false);
    let Some(id) = b.layout_runs().flat_map(|r| r.glyphs.iter().map(|g| g.font_id)).next() else { return false };
    fs.db().face(id).is_some_and(|f| f.families.iter().any(|(n, _)| n == family))
}

/// 본문·굵게·기울임에 쓸 실제 면
#[derive(Clone, Copy, PartialEq, Debug)]
struct Faces { regular: Font, bold: Font, italic: Font }

/// 패밀리 안에 실제로 있는 굵기·스타일만 고른다.
/// cosmic-text 는 요청한 굵기(예: 400)의 면이 없으면 패밀리를 통째로 건너뛰고 다른 글꼴로 대체해 버린다
/// (예: Light/Medium/Bold 만 있는 KoPub 글꼴 → 전부 Apple SD Gothic Neo 로 보임).
fn faces_of(family: &'static str) -> Faces {
    use iced::advanced::graphics::text::cosmic_text::fontdb::Style;
    let mut fs = font_system().write().expect("font system");
    let faces: Vec<(u16, bool)> = fs.raw().db().faces()
        .filter(|f| f.families.iter().any(|(n, _)| n == family))
        .map(|f| (f.weight.0, f.style != Style::Normal))
        .collect();
    // 목표 굵기에 가장 가까운 면. 같은 거리면 더 굵은 쪽(CSS 규칙과 같음)
    let pick = |target: u16, italic: bool| {
        faces.iter().filter(|(_, it)| *it == italic)
            .min_by_key(|(w, _)| (w.abs_diff(target), *w < target))
            .map(|(w, _)| *w)
    };
    let weight = |w: u16| match (w + 50) / 100 {
        0 | 1 => font::Weight::Thin, 2 => font::Weight::ExtraLight, 3 => font::Weight::Light,
        4 => font::Weight::Normal, 5 => font::Weight::Medium, 6 => font::Weight::Semibold,
        7 => font::Weight::Bold, 8 => font::Weight::ExtraBold, _ => font::Weight::Black,
    };
    let base = Font::with_name(family);
    let regular = Font { weight: weight(pick(400, false).unwrap_or(400)), ..base };
    let bold = Font { weight: weight(pick(700, false).unwrap_or(400)), ..base };
    // 기울임 면이 없으면 똑바로 둔다(다른 글꼴로 대체되는 것보다 낫다)
    let italic = pick(400, true).map_or(regular, |w| Font { weight: weight(w), style: font::Style::Italic, ..base });
    Faces { regular, bold, italic }
}

/// 추천 목록의 한 항목
#[derive(Clone, Copy)]
struct FontItem { family: &'static str, label: &'static str, face: Font, hangul: bool }

/// 글꼴에 한국어 이름이 있으면 그걸 보여 준다(예: MaruBuriOTF → 마루 부리OTF)
fn korean_name(family: &str) -> Option<&'static str> {
    use iced::advanced::graphics::text::cosmic_text::fontdb::Language;
    let mut fs = font_system().write().expect("font system");
    let face = fs.raw().db().faces().find(|f| f.families.iter().any(|(n, _)| n == family))?;
    let (name, _) = face.families.iter().find(|(n, l)| *l == Language::Korean_Korea && n != family)?;
    Some(Box::leak(name.clone().into_boxed_str()))
}

/// (그룹 이름, 항목들) — 설치된 추천 글꼴만
fn curated_fonts(families: &[&'static str]) -> Vec<(&'static str, Vec<FontItem>)> {
    CURATED.iter().map(|(group, names)| {
        let installed = names.iter().filter_map(|n| families.iter().find(|f| *f == n).copied());
        let items = installed.map(|f| FontItem { family: f, label: korean_name(f).unwrap_or(f), face: faces_of(f).regular, hangul: has_hangul(f) });
        (*group, items.collect::<Vec<_>>())
    }).filter(|(_, v)| !v.is_empty()).collect()
}

// ───────────────────────── 마크다운 강조 ─────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind { Plain, Mark, Bold, Italic, Code }

/// 한 줄 안의 `**굵게**`, `*기울임*`/`_기울임_`, `` `코드` `` 를 잘라 낸다. 닫히지 않은 기호는 그냥 글자.
fn inline_spans(s: &str) -> Vec<(Range<usize>, Kind)> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let (mut i, mut plain) = (0, 0);
    while i < b.len() {
        let (open, close, kind) = match b[i] {
            b'`' => (1, "`", Kind::Code),
            b'*' if b.get(i + 1) == Some(&b'*') => (2, "**", Kind::Bold),
            b'*' => (1, "*", Kind::Italic),
            b'_' if i == 0 || !b[i - 1].is_ascii_alphanumeric() => (1, "_", Kind::Italic),
            _ => { i += 1; continue; }
        };
        let body = i + open;
        let found = s[body..].find(close).filter(|&n| n > 0 && !s[body..].starts_with(' '));
        let Some(n) = found else { i += open; continue };
        if plain < i { out.push((plain..i, Kind::Plain)); }
        out.push((i..body, Kind::Mark));
        out.push((body..body + n, kind));
        out.push((body + n..body + n + close.len(), Kind::Mark));
        i = body + n + close.len();
        plain = i;
    }
    if plain < b.len() { out.push((plain..b.len(), Kind::Plain)); }
    out
}

/// 줄 앞 기호(제목/인용/목록)의 길이와 종류
fn line_prefix(line: &str) -> (usize, Option<&'static str>) {
    let hashes = line.bytes().take_while(|&b| b == b'#').count();
    if (1..=6).contains(&hashes) && line[hashes..].starts_with(' ') { return (hashes + 1, Some("h")); }
    if line.starts_with("> ") { return (2, Some("q")); }
    if ["- ", "* ", "+ "].iter().any(|p| line.starts_with(p)) { return (2, Some("l")); }
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    if digits > 0 && line[digits..].starts_with(". ") { return (digits + 2, Some("l")); }
    (0, None)
}

/// 커서 줄을 포함하는 문단(빈 줄로 구분)의 줄 범위 [start, end]
fn para_lines(lines: &[&str], at: usize) -> (usize, usize) {
    let blank = |i: usize| lines.get(i).is_none_or(|l| l.trim().is_empty());
    if blank(at) { return (at, at); }
    let (mut s, mut e) = (at, at);
    while s > 0 && !blank(s - 1) { s -= 1; }
    while !blank(e + 1) { e += 1; }
    (s, e)
}

/// text_editor 하이라이터 설정. 바뀌면 iced 가 전체를 다시 칠한다.
#[derive(Clone, PartialEq)]
struct HlSettings { p: Palette, faces: Faces, focus: Option<(usize, usize)> }

#[derive(Clone, Copy, Debug)]
struct Span { color: Color, font: Font }

struct Markdown { s: HlSettings, line: usize }

impl iced::advanced::text::Highlighter for Markdown {
    type Settings = HlSettings;
    type Highlight = Span;
    type Iterator<'a> = std::vec::IntoIter<(Range<usize>, Span)>;

    fn new(s: &HlSettings) -> Self { Self { s: s.clone(), line: 0 } }
    fn update(&mut self, s: &HlSettings) { self.s = s.clone(); self.line = 0; }
    fn change_line(&mut self, line: usize) { self.line = line; }
    fn current_line(&self) -> usize { self.line }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        let i = self.line;
        self.line += 1;
        let (p, Faces { regular: f, bold, italic }) = (self.s.p, self.s.faces);
        let span = |r: Range<usize>, color, font| (r, Span { color, font });

        if self.s.focus.is_some_and(|(s, e)| i < s || i > e) {
            return vec![span(0..line.len(), p.dim, f)].into_iter();
        }
        let (n, kind) = line_prefix(line);
        let mut out = vec![span(0..n, if kind == Some("q") { p.dim } else { p.accent }, f)];
        if kind == Some("h") {
            out.push(span(n..line.len(), p.strong, bold));
            return out.into_iter();
        }
        let quote = kind == Some("q");
        for (r, k) in inline_spans(&line[n..]) {
            let r = r.start + n..r.end + n;
            out.push(match k {
                Kind::Plain => span(r, p.fg, if quote { italic } else { f }),
                Kind::Mark => span(r, p.dim, f),
                Kind::Bold => span(r, p.strong, bold),
                Kind::Italic => span(r, p.fg, italic),
                Kind::Code => span(r, p.code, Font::MONOSPACE),
            });
        }
        out.into_iter()
    }
}

// ───────────────────────── 앱 ─────────────────────────

#[derive(Debug, Clone)]
enum Message {
    Edit(text_editor::Action),
    Shortcut(String, bool), // (키, shift)
    Escape,
    Typed,
    Moved,
    Tick,
    ToggleSettings,
    FontQuery(String),
    PickFont(Option<String>),
    Size(f32),
    Line(f32),
    Measure(u32),
    SetTheme(Theme),
    Focus(bool),
    Reset,
    SystemMode(iced::theme::Mode),
    CloseRequested(window::Id),
    Drag,
    Maximize,
    DismissNotice,
}

struct Writer {
    path: Option<PathBuf>,
    content: text_editor::Content,
    saved: String,
    dirty: bool,
    words: usize,
    chars: usize,
    undo: Vec<(String, text_editor::Cursor)>,
    redo: Vec<(String, text_editor::Cursor)>,
    last_edit: Instant,
    settings: Settings,
    faces: Faces, // settings.font 의 실제 면들
    families: Vec<&'static str>,
    curated: Vec<(&'static str, Vec<FontItem>)>,
    font_hangul: bool, // 고른 본문 글꼴이 한글을 직접 갖고 있는지
    font_query: String,
    show_settings: bool,
    system_dark: bool,
    chrome: Animation<bool>,
    now: Instant,
    notice: Option<String>,
}

fn family_named(families: &[&'static str], name: Option<&str>) -> &'static str {
    name.and_then(|n| families.iter().find(|f| **f == n).copied()).unwrap_or("Menlo")
}

impl Writer {
    fn boot() -> (Self, Task<Message>) {
        let path = std::env::args().nth(1).map(PathBuf::from);
        let text = path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
        let settings = load_settings();
        let now = Instant::now();
        let mut w = Self {
            path, content: text_editor::Content::with_text(&text), saved: text, dirty: false, words: 0, chars: 0,
            undo: Vec::new(), redo: Vec::new(), last_edit: now,
            faces: Faces { regular: Font::DEFAULT, bold: Font::DEFAULT, italic: Font::DEFAULT }, settings, families: font_families(), curated: Vec::new(), font_hangul: false, font_query: String::new(),
            show_settings: false, system_dark: true,
            chrome: Animation::new(true).duration(Duration::from_millis(250)), now, notice: None,
        };
        w.curated = curated_fonts(&w.families);
        w.set_font();
        w.recount();
        (w, system::theme().map(Message::SystemMode))
    }

    fn set_font(&mut self) {
        let family = family_named(&self.families, self.settings.font.as_deref());
        self.faces = faces_of(family);
        self.font_hangul = has_hangul(family);
    }

    fn title(&self) -> String {
        format!("{}{}", self.file_name(), if self.dirty { " •" } else { "" })
    }

    fn file_name(&self) -> String {
        self.path.as_ref().and_then(|p| p.file_name()).map_or("Untitled".into(), |n| n.to_string_lossy().into_owned())
    }

    fn palette(&self) -> Palette { self.settings.theme.resolve(self.system_dark).palette() }

    fn theme(&self) -> iced::Theme {
        let p = self.palette();
        iced::Theme::custom("iamWriter", iced::theme::Palette {
            background: p.bg, text: p.fg, primary: p.accent, success: p.accent, warning: p.code, danger: p.code,
        })
    }

    fn recount(&mut self) {
        let t = self.content.text();
        self.words = t.split_whitespace().count();
        self.chars = t.chars().filter(|c| !c.is_whitespace()).count();
        self.dirty = t != self.saved;
    }

    fn set_text(&mut self, text: &str) {
        self.content = text_editor::Content::with_text(text);
        self.undo.clear();
        self.redo.clear();
        self.saved = text.to_owned();
        self.recount();
    }

    fn open(&mut self) {
        if self.dirty && !self.confirm_discard() { return; }
        let Some(p) = md_dialog().pick_file() else { return };
        match std::fs::read_to_string(&p) {
            Ok(t) => { self.set_text(&t); self.path = Some(p); }
            Err(e) => error(&format!("열기 실패: {e}")),
        }
    }

    /// 저장 성공 여부 반환
    fn save(&mut self, save_as: bool) -> bool {
        let path = match (&self.path, save_as) {
            (Some(p), false) => p.clone(),
            _ => match md_dialog().set_file_name("Untitled.md").save_file() { Some(p) => p, None => return false },
        };
        let text = self.content.text();
        match std::fs::write(&path, &text) {
            Ok(()) => { self.saved = text; self.dirty = false; self.path = Some(path); true }
            Err(e) => { error(&format!("저장 실패: {e}")); false }
        }
    }

    /// 저장되지 않은 변경이 있을 때: 저장/버리기/취소. 계속 진행해도 되면 true.
    /// ponytail: rfd 동기 대화상자(메인 스레드 모달). 비동기 흐름이 필요해지면 AsyncFileDialog + Task 로.
    fn confirm_discard(&mut self) -> bool {
        use rfd::{MessageButtons, MessageDialog, MessageDialogResult as R};
        match MessageDialog::new()
            .set_title("저장되지 않은 변경")
            .set_description("변경 사항을 저장할까요?")
            .set_buttons(MessageButtons::YesNoCancel)
            .show()
        {
            R::Yes => self.save(false),
            R::No => true,
            _ => false,
        }
    }

    fn edit(&mut self, action: text_editor::Action) {
        use text_editor::{Action, Edit};
        if action.is_edit() {
            // 실행 취소 단위: 0.8초 이상 쉬었거나, 붙여넣기·줄바꿈·삭제 같은 큰 동작 직전
            let small = matches!(action, Action::Edit(Edit::Insert(_)));
            if !small || self.last_edit.elapsed() > Duration::from_millis(800) || self.undo.is_empty() {
                self.undo.push((self.content.text(), self.content.cursor()));
                if self.undo.len() > 500 { self.undo.remove(0); }
            }
            self.redo.clear();
            self.last_edit = Instant::now();
            self.content.perform(action);
            self.recount();
        } else {
            self.content.perform(action);
        }
    }

    fn undo_redo(&mut self, redo: bool) {
        let (from, to) = if redo { (&mut self.redo, &mut self.undo) } else { (&mut self.undo, &mut self.redo) };
        let Some((text, cursor)) = from.pop() else { return };
        to.push((self.content.text(), self.content.cursor()));
        self.content = text_editor::Content::with_text(&text);
        self.content.move_to(cursor);
        self.last_edit = Instant::now() - Duration::from_secs(1); // 다음 입력은 새 실행 취소 단위
        self.recount();
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        self.now = Instant::now();
        let before = self.settings.clone();
        let mut task = Task::none();
        match msg {
            Message::Edit(a) => self.edit(a),
            Message::Shortcut(k, shift) => match (k.as_str(), shift) {
                ("s", true) => { self.save(true); }
                ("s", false) => { self.save(false); }
                ("o", _) => self.open(),
                (",", _) => self.show_settings ^= true,
                ("z", s) => self.undo_redo(s),
                ("f", true) => self.settings.focus ^= true,
                ("d", true) => self.settings.theme = self.settings.theme.next(),
                _ => {}
            },
            Message::Escape => self.show_settings = false,
            // 방해 없는 화면: 타이핑하면 숨기고, 마우스를 움직이면 보인다
            Message::Typed if !self.show_settings => self.chrome.go_mut(false, self.now),
            Message::Typed => {}
            Message::Moved => self.chrome.go_mut(true, self.now),
            Message::Tick => {}
            Message::ToggleSettings => self.show_settings ^= true,
            Message::FontQuery(q) => self.font_query = q,
            Message::PickFont(f) => self.settings.font = f,
            Message::Size(v) => self.settings.size = v,
            Message::Line(v) => self.settings.line = v,
            Message::Measure(v) => self.settings.measure = v,
            Message::SetTheme(t) => self.settings.theme = t,
            Message::Focus(v) => self.settings.focus = v,
            Message::Reset => self.settings = Settings::default(),
            Message::SystemMode(m) => self.system_dark = m == iced::theme::Mode::Dark,
            Message::CloseRequested(id) => {
                if !self.dirty || self.confirm_discard() { task = window::close(id); }
            }
            Message::Drag => task = window::latest().and_then(window::drag),
            Message::Maximize => task = window::latest().and_then(window::toggle_maximize),
            Message::DismissNotice => self.notice = None,
        }
        if self.settings != before {
            if self.settings.font != before.font { self.set_font(); }
            store_settings(&self.settings);
        }
        task
    }

    fn subscription(&self) -> Subscription<Message> {
        let events = event::listen_with(|e, _status, _id| match e {
            Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => match key.as_ref() {
                keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::Escape),
                keyboard::Key::Character(c) if modifiers.command() => Some(Message::Shortcut(c.to_lowercase(), modifiers.shift())),
                _ if modifiers.command() => None,
                _ => Some(Message::Typed),
            },
            Event::Mouse(mouse::Event::CursorMoved { .. }) => Some(Message::Moved),
            _ => None,
        });
        let frames = if self.chrome.is_animating(self.now) { window::frames().map(|_| Message::Tick) } else { Subscription::none() };
        Subscription::batch([
            events,
            frames,
            window::close_requests().map(Message::CloseRequested),
            system::theme_changes().map(Message::SystemMode),
        ])
    }

    fn view(&self) -> Element<'_, Message> {
        let p = self.palette();
        let s = &self.settings;
        let alpha = self.chrome.interpolate(0.0, 1.0, self.now);
        let faint = |c: Color| Color { a: c.a * alpha, ..c };

        // 타이틀바 자리: 끌어서 창 이동, 더블클릭으로 확대, 가운데 파일 이름
        let titlebar = mouse_area(
            container(text(self.title()).size(13).color(faint(p.dim))).center(Length::Fill).height(TITLEBAR),
        ).on_press(Message::Drag).on_double_click(Message::Maximize);

        // 본문
        let lines: Vec<String> = self.content.lines().map(|l| l.text.into_owned()).collect();
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        let focus = s.focus.then(|| para_lines(&refs, self.content.cursor().position.line));
        // ponytail: 본문 폭은 '0' 폭을 0.6em 으로 어림한다(고정폭 기준). 비례 글꼴에선 조금 넓거나 좁다.
        let width = s.measure as f32 * s.size * 0.6;
        let editor = text_editor(&self.content)
            .on_action(Message::Edit)
            .font(self.faces.regular)
            .size(s.size)
            .line_height(s.line)
            .height(Length::Fill)
            .padding(Padding { top: 48.0, bottom: 160.0, left: 0.0, right: 0.0 })
            .key_binding(|kp| match kp.key.as_ref() {
                // ⌘ 조합은 앱 단축키로. 복사/잘라내기/붙여넣기/전체 선택만 편집기에 남긴다
                keyboard::Key::Character(c) if kp.modifiers.command() && !["c", "x", "v", "a"].contains(&c.to_lowercase().as_str()) => None,
                _ => text_editor::Binding::from_key_press(kp),
            })
            .style(move |_, _| text_editor::Style {
                background: Color::TRANSPARENT.into(),
                border: Border::default(),
                placeholder: p.dim,
                value: p.accent, // 모든 글자는 하이라이터가 칠하므로 이 색은 커서에만 쓰인다
                selection: Color { a: 0.3, ..p.accent },
            })
            .placeholder("쓰기 시작하세요…")
            .highlight_with::<Markdown>(HlSettings { p, faces: self.faces, focus }, |span: &Span, _: &iced::Theme| iced::advanced::text::highlighter::Format { color: Some(span.color), font: Some(span.font) });
        let editor = ImeFix { inner: editor.into(), font: self.faces.regular, size: s.size, line: s.line, fg: p.strong, bg: p.bg, accent: p.accent };
        let body = container(container(Element::from(editor)).max_width(width)).center_x(Length::Fill).height(Length::Fill)
            .padding(Padding { left: 24.0, right: 24.0, ..Padding::ZERO });

        // 상태 표시줄
        let modes = if s.focus { "포커스" } else { "" };
        let mut status = row![text(modes).size(12).color(faint(p.dim))].spacing(12).align_y(iced::Alignment::Center);
        if let Some(n) = &self.notice {
            status = status.push(mouse_area(text(format!("⚠ {n}")).size(12).color(p.code)).on_press(Message::DismissNotice));
        }
        status = status
            .push(space::horizontal())
            .push(text(format!("{} 단어 · {} 자 · {}분", self.words, self.chars, (self.words / 200).max(1))).size(12).color(faint(p.dim)))
            .push(mouse_area(text("⚙").size(15).color(faint(p.dim))).on_press(Message::ToggleSettings).interaction(mouse::Interaction::Pointer));
        let status = container(status).padding(Padding { top: 6.0, bottom: 10.0, left: 16.0, right: 16.0 });

        let main = container(column![titlebar, body, status])
            .style(move |_| container::Style { background: Some(p.bg.into()), ..Default::default() })
            .width(Length::Fill).height(Length::Fill);

        if !self.show_settings { return main.into(); }
        // 설정: 오른쪽 위에 떠 있는 카드
        let card = container(opaque(self.settings_card(p)))
            .align_right(Length::Fill)
            .padding(Padding { top: TITLEBAR + 8.0, right: 16.0, bottom: 48.0, left: 0.0 });
        stack![main, card].into()
    }

    fn settings_card(&self, p: Palette) -> Element<'_, Message> {
        let s = &self.settings;
        let label = |t| text(t).size(12).color(p.dim);
        let q = self.font_query.to_lowercase();
        let current = s.font.clone().unwrap_or_else(|| "Menlo".into());
        let current = self.curated.iter().flat_map(|(_, v)| v).find(|it| it.family == current).map_or(current, |it| it.label.to_owned());
        let item = |face: Font, label: &'static str, value: Option<String>, hangul: bool| {
            let selected = value == s.font;
            let mut r = row![text(label).size(13).font(face)].align_y(iced::Alignment::Center);
            if hangul { r = r.push(space::horizontal()).push(text("가").size(11).color(p.dim)); }
            button(r)
                .on_press(Message::PickFont(value))
                .width(Length::Fill)
                .style(move |_, st| button::Style {
                    background: (selected || st == button::Status::Hovered).then(|| Color { a: if selected { 0.25 } else { 0.1 }, ..p.accent }.into()),
                    text_color: p.fg,
                    border: Border::default().rounded(4),
                    ..Default::default()
                })
        };
        let group = |name: &'static str| Element::from(container(text(name).size(11).color(p.dim)).padding(Padding { top: 8.0, bottom: 2.0, left: 6.0, right: 0.0 }));
        let list: Vec<Element<'_, Message>> = if q.is_empty() {
            let mut v = vec![item(Font::with_name("Menlo"), "Menlo (기본)", None, false).into()];
            for (g, fonts) in &self.curated {
                v.push(group(g));
                v.extend(fonts.iter().map(|it| item(it.face, it.label, Some(it.family.to_owned()), it.hangul).into()));
            }
            v
        } else {
            // 검색은 설치된 모든 글꼴에서. 한글 포함 여부는 비용 때문에 추천 목록에만 표시
            let hits = self.families.iter().filter(|n| n.to_lowercase().contains(&q)).take(200);
            hits.map(|&f| item(Font::with_name(f), f, Some(f.to_owned()), false).into()).collect()
        };
        let list = column(list).spacing(1);

        let themes = row(Theme::ALL.iter().map(|&(t, name)| {
            let on = s.theme == t;
            button(text(name).size(12)).on_press(Message::SetTheme(t)).padding([4, 8])
                .style(move |_, _| button::Style {
                    background: Some(if on { p.accent } else { Color { a: 0.08, ..p.fg } }.into()),
                    text_color: if on { Color::WHITE } else { p.fg },
                    border: Border::default().rounded(12),
                    ..Default::default()
                })
                .into()
        })).spacing(4).wrap().vertical_spacing(4); // 한글 이름이 길어도 카드 밖으로 넘치지 않게

        let slider_row = |name: &'static str, value: String, s: Element<'static, Message>| {
            column![row![label(name), space::horizontal(), text(value).size(12).color(p.dim)], s].spacing(4)
        };

        container(column![
            text("설정").size(16).color(p.strong),
            label("글꼴"),
            text(current).size(18).font(self.faces.regular).color(p.strong),
            text("다람쥐 헌 쳇바퀴에 타고파 · The quick brown fox").size(14).font(self.faces.regular).color(p.fg),
            text(if self.font_hangul { "한글 포함 글꼴" } else { "이 글꼴엔 한글이 없어 한글은 시스템 글꼴로 표시돼요" }).size(11).color(p.dim),
            text_input("모든 글꼴에서 검색…", &self.font_query).on_input(Message::FontQuery).size(13).padding(6),
            scrollable(list).height(220).spacing(6),
            slider_row("크기", format!("{:.0}", s.size), slider(12.0..=32.0, s.size, Message::Size).step(1.0).into()),
            slider_row("줄간격", format!("{:.2}", s.line), slider(1.2..=2.4, s.line, Message::Line).step(0.05).into()),
            slider_row("본문 폭 (글자)", s.measure.to_string(), slider(40..=100, s.measure, Message::Measure).into()),
            label("테마"),
            themes,
            toggler(s.focus).label("포커스 모드  ⇧⌘F").on_toggle(Message::Focus).size(16).text_size(13),
            button(text("기본값으로").size(12)).on_press(Message::Reset).style(button::text),
        ].spacing(10))
        .width(300)
        .padding(18)
        .style(move |_| container::Style {
            background: Some(p.panel.into()),
            border: Border::default().rounded(12).width(1).color(Color { a: 0.08, ..p.fg }),
            shadow: iced::Shadow { color: Color { a: 0.25, ..Color::BLACK }, offset: iced::Vector::new(0.0, 8.0), blur_radius: 24.0 },
            ..Default::default()
        })
        .into()
    }
}

// ───────────────────────── 한글 조합 표시 보정 ─────────────────────────

/// 한글을 조합하는 동안(ㅎ → 하 → 한) iced 는 조합 중인 글자를 커서 칸 **맨 아래**에 UI 글꼴로 띄운다.
/// 줄간격이 1.3 보다 크면 글자가 줄 아래로 처져 보이고, 본문 글꼴과도 달라서 확정되는 순간 위로 튄다.
/// 이 래퍼는 iced 의 오버레이를 끄고, 조합 중인 글자를 본문과 같은 글꼴·같은 줄 위치에 직접 그린다.
struct ImeFix<'a> {
    inner: Element<'a, Message>,
    font: Font,
    size: f32,
    line: f32,
    fg: Color,
    bg: Color,
    accent: Color,
}

#[derive(Default)]
struct ImeState { preedit: Option<(String, iced::Rectangle)> } // (조합 중인 글자, 커서 칸)

mod ime_fix {
    use super::*;
    use iced::advanced::{
        layout, overlay, renderer,
        text::{self as adv_text, Paragraph as _, Renderer as _},
        widget::{tree, Operation, Tree, Widget},
        Clipboard, Layout, Renderer as _, Shell,
    };
    use iced::advanced::InputMethod;
    use iced::{Pixels, Rectangle, Size, Vector};

    type R = iced::Renderer;

    impl Widget<Message, iced::Theme, R> for ImeFix<'_> {
        fn size(&self) -> Size<Length> { self.inner.as_widget().size() }
        fn size_hint(&self) -> Size<Length> { self.inner.as_widget().size_hint() }
        fn tag(&self) -> tree::Tag { tree::Tag::of::<ImeState>() }
        fn state(&self) -> tree::State { tree::State::new(ImeState::default()) }
        fn children(&self) -> Vec<Tree> { vec![Tree::new(&self.inner)] }
        fn diff(&self, tree: &mut Tree) { tree.diff_children(std::slice::from_ref(&self.inner)); }

        fn layout(&mut self, tree: &mut Tree, renderer: &R, limits: &layout::Limits) -> layout::Node {
            self.inner.as_widget_mut().layout(&mut tree.children[0], renderer, limits)
        }

        fn operate(&mut self, tree: &mut Tree, layout: Layout<'_>, renderer: &R, op: &mut dyn Operation) {
            self.inner.as_widget_mut().operate(&mut tree.children[0], layout, renderer, op);
        }

        fn update(
            &mut self, tree: &mut Tree, event: &iced::Event, layout: Layout<'_>, cursor: mouse::Cursor,
            renderer: &R, clipboard: &mut dyn Clipboard, shell: &mut Shell<'_, Message>, viewport: &Rectangle,
        ) {
            self.inner.as_widget_mut().update(&mut tree.children[0], event, layout, cursor, renderer, clipboard, shell, viewport);
            let focused = tree.children[0].state.downcast_ref::<text_editor::State<Markdown>>().is_focused();
            let state = tree.state.downcast_mut::<ImeState>();
            if !focused {
                state.preedit = None;
            } else if let InputMethod::Enabled { cursor, preedit, .. } = shell.input_method_mut() {
                // 조합 중 글자를 가져오고 iced 오버레이는 끈다. IME 자체(후보창 위치 등)는 그대로 둔다
                let new = preedit.take().filter(|p| !p.content.is_empty()).map(|p| (p.content, *cursor));
                if new != state.preedit { shell.request_redraw(); }
                state.preedit = new;
            }
        }

        fn draw(
            &self, tree: &Tree, renderer: &mut R, theme: &iced::Theme, style: &renderer::Style,
            layout: Layout<'_>, cursor: mouse::Cursor, viewport: &Rectangle,
        ) {
            self.inner.as_widget().draw(&tree.children[0], renderer, theme, style, layout, cursor, viewport);
            let Some((content, caret)) = &tree.state.downcast_ref::<ImeState>().preedit else { return };
            let text = adv_text::Text {
                content: content.clone(),
                bounds: Size::INFINITE,
                size: Pixels(self.size),
                line_height: adv_text::LineHeight::Relative(self.line), // 본문과 같은 줄 높이 → 같은 기준선
                font: self.font,
                align_x: adv_text::Alignment::Default,
                align_y: iced::alignment::Vertical::Top,
                shaping: adv_text::Shaping::Advanced,
                wrapping: adv_text::Wrapping::None,
            };
            // 폭 재기용 paragraph. 그리기엔 쓰지 않는다: 렌더러는 paragraph 를 약한 참조로만 들고 있어서
            // draw 가 끝나 지역 변수가 사라지면 글자가 안 그려진다. fill_text 는 글자를 직접 소유한다.
            let width = <R as adv_text::Renderer>::Paragraph::with_text(text.with_content(content.as_str())).min_width();
            let bounds = Rectangle::new(caret.position(), Size::new(width, caret.height));
            // 새 레이어: 같은 레이어에선 글자가 사각형보다 나중에 그려져 뒤 글자가 비친다
            renderer.with_layer(*viewport, |r| {
                r.fill_quad(renderer::Quad { bounds, ..Default::default() }, self.bg);
                r.fill_text(text, bounds.position(), self.fg, bounds);
                let underline = Rectangle::new(bounds.position() + Vector::new(0.0, bounds.height - 4.0), Size::new(bounds.width, 1.5));
                r.fill_quad(renderer::Quad { bounds: underline, ..Default::default() }, self.accent);
            });
        }

        fn mouse_interaction(&self, tree: &Tree, layout: Layout<'_>, cursor: mouse::Cursor, viewport: &Rectangle, renderer: &R) -> mouse::Interaction {
            self.inner.as_widget().mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
        }

        fn overlay<'b>(
            &'b mut self, tree: &'b mut Tree, layout: Layout<'b>, renderer: &R, viewport: &Rectangle, translation: Vector,
        ) -> Option<overlay::Element<'b, Message, iced::Theme, R>> {
            self.inner.as_widget_mut().overlay(&mut tree.children[0], layout, renderer, viewport, translation)
        }
    }

    impl<'a> From<ImeFix<'a>> for Element<'a, Message> {
        fn from(w: ImeFix<'a>) -> Self { Element::new(w) }
    }
}

fn md_dialog() -> rfd::FileDialog {
    rfd::FileDialog::new().add_filter("Markdown", &["md", "markdown", "txt"])
}

fn error(msg: &str) {
    rfd::MessageDialog::new().set_level(rfd::MessageLevel::Error).set_description(msg).show();
}

#[test]
fn para_lines_works() {
    let l = ["one", "", "two", "lines", "", "three"];
    assert_eq!(para_lines(&l, 0), (0, 0));
    assert_eq!(para_lines(&l, 3), (2, 3));
    assert_eq!(para_lines(&l, 1), (1, 1));
    assert_eq!(para_lines(&l, 5), (5, 5));
}

#[test]
fn inline_spans_works() {
    let kinds = |s: &str| inline_spans(s).into_iter().map(|(r, k)| (s[r].to_string(), k)).collect::<Vec<_>>();
    use Kind::*;
    assert_eq!(kinds("**굵게** 와 *기울임* `코드`"), vec![
        ("**".into(), Mark), ("굵게".into(), Bold), ("**".into(), Mark), (" 와 ".into(), Plain),
        ("*".into(), Mark), ("기울임".into(), Italic), ("*".into(), Mark), (" ".into(), Plain),
        ("`".into(), Mark), ("코드".into(), Code), ("`".into(), Mark),
    ]);
    assert_eq!(kinds("닫히지 않은 **별"), vec![("닫히지 않은 **별".into(), Plain)]);
    assert_eq!(kinds("snake_case_name"), vec![("snake_case_name".into(), Plain)]);
    assert_eq!(kinds("2 * 3 * 4"), vec![("2 * 3 * 4".into(), Plain)]);
    assert_eq!(kinds(""), vec![]);
}

#[test]
fn korean_renders_without_tofu() {
    // 한글이 없는 Menlo 와 UI 글꼴 모두에서 '한글' 이 .notdef(0) 없이 그려져야 한다(대체 글꼴 동작)
    use iced::advanced::graphics::text::cosmic_text::{Attrs, Buffer, Family, Metrics, Shaping};
    assert!(has_hangul("Apple SD Gothic Neo"));
    assert!(!has_hangul("Menlo"));
    // Regular 가 없는 글꼴도 실제 있는 굵기를 골라야 한다
    let f = faces_of("Menlo");
    assert_eq!((f.regular.weight, f.bold.weight, f.italic.style), (font::Weight::Normal, font::Weight::Bold, font::Style::Italic));
    let mut fs = font_system().write().unwrap();
    let fs = fs.raw();
    for family in ["Menlo", "Apple SD Gothic Neo"] {
        let mut b = Buffer::new(fs, Metrics::new(16.0, 20.0));
        b.set_text(fs, "한글 입력 Abc", &Attrs::new().family(Family::Name(family)), Shaping::Advanced, None);
        b.shape_until_scroll(fs, false);
        let glyphs: Vec<_> = b.layout_runs().flat_map(|r| r.glyphs.iter().map(|g| g.glyph_id)).collect();
        assert!(!glyphs.is_empty() && glyphs.iter().all(|&g| g != 0), "{family}: {glyphs:?}");
    }
}
