// 영어 공부 모드 — typingEng(따라치기 + 영작 첨삭) 이식
// 원본: github.com/1ncarnati0n/typingEng. 이식하며 달라진 곳은 docs/TYPINGENG_PORT.md 5장,
// 이식 뒤 새로 넣은 기능(다시 쓰기 · 오류 유형 · 복습)은 같은 문서 11장에 있다.
mod claude;
mod player;
mod store;
mod typing;
mod youtube;

use crate::theme::{Palette, TITLEBAR, UI_FONT};
use claude::{Engine, Kind, Line, Review};
use iced::widget::{
    Column, button, column, container, mouse_area, opaque, operation, progress_bar, rich_text, row,
    scrollable, space, span, stack, text, text_editor, text_input,
};
use iced::{
    Border, Center, Color, Element, Event, Fill, Font, Padding, Shadow, Subscription, Task, Vector,
};
use iced::{event, keyboard, window};
use player::Player;
use std::collections::HashMap;
use std::time::Instant;
use store::{Lesson, Library, Stats, Video, today};
use typing::CharState;

const LEVELS: [&str; 4] = ["A1", "A2", "B1", "B2"];
const INPUT_ID: &str = "typing";
const WRITE_ID: &str = "writing";
const SCROLL_ID: &str = "practice-scroll";
/// 한 번에 복습할 문장 수 (직접 쓰기 한 화면 분량)
const REVIEW_MAX: usize = 10;

/// (그룹 이름, [(패밀리 이름, 보여줄 이름)]). 같은 글꼴의 다른 패밀리 이름은 같은 라벨로 묶는다.
type FontGroup = (&'static str, &'static [(&'static str, &'static str)]);
/// (패밀리, 라벨, 설치됨)
type FontItem = (&'static str, &'static str, bool);
type FontGroups = Vec<(&'static str, Vec<FontItem>)>;

const EN_DEFAULT: &str = "Menlo";
const KO_DEFAULT: &str = "Apple SD Gothic Neo";

/// 한글 글꼴: 명조 · 고딕 · 고정폭(코딩 글꼴 + 한글 조합)
const KO_FONTS: &[FontGroup] = &[
    (
        "명조",
        &[
            ("MaruBuri", "마루 부리"),
            ("MaruBuriOTF", "마루 부리"),
            ("Gowun Batang", "고운바탕"),
            ("RIDIBatang", "리디바탕"),
        ],
    ),
    (
        "고딕",
        &[
            ("Pretendard", "프리텐다드"),
            ("Gowun Dodum", "고운돋움"),
            ("Source Han Sans K", "본고딕"),
            ("Source Han Sans KR", "본고딕"),
            ("Noto Sans KR", "본고딕"),
            ("Noto Sans CJK KR", "본고딕"),
            ("Apple SD Gothic Neo", "애플 SD 고딕 (기본)"),
        ],
    ),
    (
        "고정폭",
        &[
            ("D2Coding", "D2Coding"),
            ("D2Coding ligature", "D2Coding"),
            ("Jetendard", "Jetendard"),
            ("Monatendard", "Monatendard"),
        ],
    ),
];

/// 영문 글꼴: 위 한글 글꼴과 짝이 되는 것끼리 같은 그룹
const EN_FONTS: &[FontGroup] = &[
    (
        "세리프 — 마루 부리 · 고운바탕 · 리디바탕과 짝",
        &[
            ("New York", "New York"),
            ("Source Serif 4", "Source Serif"),
            ("Source Serif Pro", "Source Serif"),
            ("Charter", "Charter"),
            ("Iowan Old Style", "Iowan Old Style"),
        ],
    ),
    (
        "산세리프 — 프리텐다드 · 본고딕 · 고운돋움과 짝",
        &[
            ("Inter", "Inter"),
            ("Pretendard", "Pretendard"),
            ("Source Sans 3", "Source Sans"),
            ("Source Sans Pro", "Source Sans"),
            ("Helvetica Neue", "Helvetica Neue"),
        ],
    ),
    (
        "고정폭 — 따라치기에 추천",
        &[
            ("JetBrains Mono", "JetBrains Mono"),
            ("Jetendard", "Jetendard"),
            ("Monaspace Neon", "Monaspace Neon"),
            ("Monatendard", "Monatendard"),
            ("D2Coding", "D2Coding"),
            ("D2Coding ligature", "D2Coding"),
            ("Source Code Pro", "Source Code Pro"),
            ("Menlo", "Menlo (기본)"),
        ],
    ),
];

/// 그룹마다 설치 여부를 붙인다. 같은 라벨이 여럿이면 설치된 패밀리를 남긴다.
fn font_groups(groups: &[FontGroup]) -> Vec<(&'static str, Vec<FontItem>)> {
    let mut fs = iced::advanced::graphics::text::font_system()
        .write()
        .expect("font system");
    let db = fs.raw().db();
    let has = |n: &str| {
        db.faces()
            .any(|f| f.families.iter().any(|(fam, _)| fam == n))
    };
    groups
        .iter()
        .map(|&(g, list)| {
            let mut items: Vec<FontItem> = vec![];
            for &(family, label) in list {
                let ok = has(family);
                match items.iter_mut().find(|i| i.1 == label) {
                    Some(i) if ok && !i.2 => *i = (family, label, ok),
                    Some(_) => {}
                    None => items.push((family, label, ok)),
                }
            }
            (g, items)
        })
        .collect()
}

/// 저장된 이름이 목록에 있고 설치돼 있으면 그 글꼴, 아니면 기본값.
fn pick_font(
    groups: &[(&'static str, Vec<FontItem>)],
    saved: &Option<String>,
    default: &'static str,
) -> Font {
    let found = groups
        .iter()
        .flat_map(|(_, v)| v)
        .find(|i| i.2 && saved.as_deref() == Some(i.0));
    Font::with_name(found.map_or(default, |i| i.0))
}

// ---- 상태 ----

#[derive(PartialEq)]
enum Screen {
    Setup,
    Typing,
    Writing,
}

pub(crate) struct Practice {
    stats: Stats,
    library: Library,
    screen: Screen,
    level: &'static str,
    chat: bool,
    topic: String,
    lines: Vec<Line>,
    idx: usize,
    typed: String,
    line_start: Option<Instant>,
    total_secs: f64,
    total_correct: usize,
    total_chars: usize,
    editor: text_editor::Content,
    review: Option<Review>,
    /// 다시 쓰기 중이면 처음 쓴 글과 그 첨삭. 이때 editor·review 는 다시 쓴 글의 것.
    draft: Option<(String, Review)>,
    /// 복습 중이면 지금 lines 가 온 복습 카드 번호(lines 와 같은 순서). 비었으면 보통 연습.
    review_cards: Vec<usize>,
    /// 설정 카드(AI 엔진 · 유튜브 쿠키 · 글꼴)가 열려 있는지
    show_settings: bool,
    /// "정말 삭제?"를 묻고 있는 보관함 항목
    confirm_delete: Option<Del>,
    /// 작업 중이면 안내 문구
    busy: Option<String>,
    error: Option<String>,
    yt_url: String,
    /// 지금 치고 있는 유튜브 세트 (영상 ID, 세트 번호). AI 문장이면 None.
    cur_video: Option<(String, usize)>,
    player: Option<Player>,
    slow: bool,
    /// 설치된 추천 글꼴 (영문, 한글)
    fonts: (FontGroups, FontGroups),
    /// 영어 공부 모드가 화면에 보이는지. 숨어 있을 땐 포커스·재생 같은 부수효과를 미룬다(resume).
    pub(crate) visible: bool,
}

#[derive(Debug, Clone)]
pub(crate) enum Msg {
    Level(&'static str),
    Chat(bool),
    Topic(String),
    Generate,
    Generated(Result<Vec<Line>, String>),
    Typed(String),
    Submit,
    Edit(text_editor::Action),
    AskReview,
    Reviewed(Result<Review, String>),
    StartRewrite,
    StartReview,
    ToggleSettings,
    AskDelete(Del),
    CancelDelete,
    Home,
    YtUrl(String),
    YtLoad,
    YtLoaded(Result<youtube::Transcript, String>),
    OpenSet(String, usize),
    SetReady(String, usize, Result<Vec<Line>, String>),
    OpenLesson(usize),
    DeleteLesson(usize),
    DeleteVideo(String),
    Replay,
    OpenPlayer,
    Slow(bool),
    Cookies(bool),
    Drag,
    Maximize,
    Escape,
    DismissError,
    Skip,
    Engine(Engine),
    EnFont(&'static str),
    KoFont(&'static str),
}

/// 보관함에서 지울 항목
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Del {
    Video(String),
    Lesson(usize),
}

/// claude / yt-dlp 호출은 블로킹이라 별도 스레드에서 돌리고 결과만 future로 받는다.
fn off_thread<T: Send + 'static>(
    f: impl FnOnce() -> T + Send + 'static,
) -> impl Future<Output = T> {
    let (tx, rx) = iced::futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    async move { rx.await.expect("worker thread panicked") }
}

fn mmss(secs: f64) -> String {
    let s = secs.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

impl Practice {
    pub(crate) fn new() -> Self {
        Self {
            stats: Stats::load(),
            library: Library::load(),
            screen: Screen::Setup,
            level: "A2",
            chat: true,
            topic: "ordering coffee".into(),
            lines: vec![],
            idx: 0,
            typed: String::new(),
            line_start: None,
            total_secs: 0.0,
            total_correct: 0,
            total_chars: 0,
            editor: text_editor::Content::new(),
            review: None,
            draft: None,
            review_cards: vec![],
            show_settings: false,
            confirm_delete: None,
            busy: None,
            error: None,
            yt_url: String::new(),
            cur_video: None,
            player: None,
            slow: false,
            fonts: (font_groups(EN_FONTS), font_groups(KO_FONTS)),
            visible: false, // 앱은 집중 글쓰기 모드로 시작한다
        }
    }

    /// "Codex(gpt-6.1-terra)가 … 중…" 같은 작업 안내.
    fn working(&self, what: &str) -> String {
        let e = self.stats.engine;
        format!("{} · {} — {what}…", e.name(), e.model_label())
    }

    fn en_font(&self) -> Font {
        pick_font(&self.fonts.0, &self.stats.en_font, EN_DEFAULT)
    }

    fn ko_font(&self) -> Font {
        pick_font(&self.fonts.1, &self.stats.ko_font, KO_DEFAULT)
    }

    /// Esc·Tab·⌘, 만. 창 닫기 · 시스템 테마 구독은 앱 셸(main.rs)이 한다.
    pub(crate) fn subscription(&self) -> Subscription<Msg> {
        event::listen_with(|e, _, _| match e {
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                ..
            }) => Some(Msg::Escape),
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Tab),
                ..
            }) => Some(Msg::Skip),
            // ⌘, 설정 — 집중 글쓰기와 같은 키. 한글 자판에서도 물리 키 기준
            Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                physical_key,
                modifiers,
                ..
            }) if modifiers.command() && key.to_latin(physical_key) == Some(',') => {
                Some(Msg::ToggleSettings)
            }
            _ => None,
        })
    }

    pub(crate) fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Level(l) => self.level = l,
            Msg::Chat(c) => self.chat = c,
            Msg::Topic(t) => self.topic = t,
            Msg::Generate => {
                self.busy = Some(self.working("문장을 쓰는 중"));
                self.error = None;
                let (engine, level, chat, topic) =
                    (self.stats.engine, self.level, self.chat, self.topic.clone());
                return Task::perform(
                    off_thread(move || claude::generate_lines(engine, level, chat, &topic)),
                    Msg::Generated,
                );
            }
            Msg::Generated(res) => {
                self.busy = None;
                match res {
                    Ok(lines) => {
                        let genre = if self.chat { "채팅" } else { "소설" };
                        let title = format!("{} · {genre} · {}", self.level, self.topic.trim());
                        self.library.lessons.insert(
                            0,
                            Lesson {
                                title,
                                lines: lines.clone(),
                            },
                        );
                        self.library.save();
                        self.cur_video = None;
                        return self.begin(lines);
                    }
                    Err(e) => self.error = Some(e),
                }
            }
            Msg::Typed(s) => {
                if self.line_start.is_none() && !s.is_empty() {
                    self.line_start = Some(Instant::now());
                }
                self.typed = s;
            }
            Msg::Submit => {
                if self.typed.is_empty() {
                    self.play_current(); // 빈 칸에서 Enter = 다시 듣기
                } else if self.typed.chars().count() >= self.lines[self.idx].en.chars().count() {
                    self.finish_line(false);
                    if self.screen == Screen::Writing {
                        return self.show_writing();
                    }
                    self.play_current(); // 다음 문장 재생
                }
            }
            Msg::Skip => {
                if self.screen == Screen::Typing && self.busy.is_none() {
                    self.finish_line(true);
                    if self.screen == Screen::Writing {
                        return self.show_writing();
                    }
                    self.play_current();
                    return operation::focus(INPUT_ID);
                }
            }
            Msg::Engine(e) => {
                self.stats.engine = e;
                self.stats.save();
            }
            Msg::EnFont(f) => {
                self.stats.en_font = Some(f.into());
                self.stats.save();
            }
            Msg::KoFont(f) => {
                self.stats.ko_font = Some(f.into());
                self.stats.save();
            }
            Msg::Edit(a) => self.editor.perform(a),
            Msg::AskReview => {
                self.busy = Some(self.working("첨삭하는 중"));
                self.error = None;
                let (engine, lines, written) =
                    (self.stats.engine, self.lines.clone(), self.editor.text());
                return Task::perform(
                    off_thread(move || claude::review_writing(engine, &lines, &written)),
                    Msg::Reviewed,
                );
            }
            Msg::Reviewed(res) => {
                self.busy = None;
                match res {
                    Ok(r) => {
                        // 처음 쓴 글의 첨삭만 기록·복습 일정에 넣는다 (다시 쓴 글은 연습이라 두 번 세지 않는다)
                        if self.draft.is_none() {
                            self.record(&r);
                        }
                        self.review = Some(r);
                        return to_top(); // 첨삭 카드는 화면 맨 위에 나온다
                    }
                    Err(e) => self.error = Some(e),
                }
            }
            Msg::StartRewrite => {
                if let Some(r) = self.review.take() {
                    self.draft = Some((self.editor.text(), r));
                    self.editor = text_editor::Content::new();
                    return self.show_writing();
                }
            }
            Msg::StartReview => {
                let cards = self.library.due(today(), REVIEW_MAX);
                if !cards.is_empty() {
                    let lines = cards
                        .iter()
                        .map(|&i| self.library.deck[i].line.clone())
                        .collect();
                    self.cur_video = None;
                    self.reset(lines);
                    self.review_cards = cards;
                    self.screen = Screen::Writing; // 따라치기 없이 바로 영작
                    return self.show_writing();
                }
            }
            Msg::Home => {
                self.screen = Screen::Setup;
                self.error = None;
                return to_top();
            }
            Msg::YtUrl(u) => self.yt_url = u,
            Msg::YtLoad => {
                self.error = None;
                // 보관함에 있는 영상이면 다시 받지 않고 이어서
                if let Some(v) =
                    youtube::video_id(&self.yt_url).and_then(|id| self.library.video(&id))
                {
                    let msg = Msg::OpenSet(v.id.clone(), resume_set(v));
                    self.yt_url.clear();
                    return self.update(msg);
                }
                self.busy = Some("자막을 가져오는 중…".into());
                let (url, cookies) = (self.yt_url.clone(), !self.stats.no_yt_cookies);
                return Task::perform(
                    off_thread(move || youtube::fetch_transcript(&url, cookies)),
                    Msg::YtLoaded,
                );
            }
            Msg::YtLoaded(res) => match res {
                Ok(t) => {
                    let chunks = youtube::chunks(&t.words, t.end, youtube::CHUNK_WORDS);
                    self.library.videos.retain(|v| v.id != t.id);
                    self.library.videos.insert(
                        0,
                        Video {
                            id: t.id.clone(),
                            title: t.title,
                            sets: vec![None; chunks.len()],
                            chunks,
                            progress: 0,
                        },
                    );
                    self.library.save();
                    self.yt_url.clear();
                    return self.update(Msg::OpenSet(t.id, 0));
                }
                Err(e) => {
                    self.busy = None;
                    self.error = Some(e);
                }
            },
            Msg::OpenSet(id, i) => {
                let Some(v) = self.library.video(&id) else {
                    return Task::none();
                };
                if let Some(Some(lines)) = v.sets.get(i) {
                    let lines = lines.clone();
                    self.busy = None;
                    self.cur_video = Some((id, i));
                    return self.begin(lines);
                }
                let Some(chunk) = v.chunks.get(i).cloned() else {
                    return Task::none();
                };
                self.busy = Some(self.working("자막을 문장으로 정리하는 중"));
                self.error = None;
                let engine = self.stats.engine;
                return Task::perform(
                    off_thread(move || {
                        claude::lines_from_transcript(engine, &chunk.text()).map(|mut lines| {
                            youtube::align(&chunk, &mut lines);
                            lines
                        })
                    }),
                    move |r| Msg::SetReady(id.clone(), i, r),
                );
            }
            Msg::SetReady(id, i, res) => {
                self.busy = None;
                match res {
                    Ok(lines) => {
                        if let Some(set) =
                            self.library.video_mut(&id).and_then(|v| v.sets.get_mut(i))
                        {
                            *set = Some(lines.clone());
                        }
                        self.library.save();
                        self.cur_video = Some((id, i));
                        return self.begin(lines);
                    }
                    Err(e) => self.error = Some(e),
                }
            }
            Msg::OpenLesson(i) => {
                if let Some(l) = self.library.lessons.get(i) {
                    let lines = l.lines.clone();
                    self.cur_video = None;
                    return self.begin(lines);
                }
            }
            Msg::ToggleSettings => self.show_settings = !self.show_settings,
            Msg::AskDelete(d) => self.confirm_delete = Some(d),
            Msg::CancelDelete => self.confirm_delete = None,
            Msg::DeleteLesson(i) => {
                self.confirm_delete = None;
                if i < self.library.lessons.len() {
                    self.library.lessons.remove(i);
                    self.library.save();
                }
            }
            Msg::DeleteVideo(id) => {
                self.confirm_delete = None;
                self.library.videos.retain(|v| v.id != id);
                self.library.save();
            }
            Msg::Replay => {
                self.play_current();
                return operation::focus(INPUT_ID);
            }
            Msg::OpenPlayer => {
                match &self.player {
                    Some(p) => {
                        if let Err(e) = youtube::open_in_browser(&p.url()) {
                            self.error = Some(e);
                        }
                    }
                    None => self.play_current(), // 처음이면 서버를 띄우고 브라우저를 연다
                }
                return operation::focus(INPUT_ID);
            }
            Msg::Cookies(on) => {
                self.stats.no_yt_cookies = !on;
                self.stats.save();
            }
            Msg::Drag => return window::latest().and_then(window::drag),
            Msg::Maximize => return window::latest().and_then(window::toggle_maximize),
            Msg::DismissError => self.error = None,
            // Esc 는 가장 위에 떠 있는 것부터 닫는다: 오류 → 설정 카드 → 따라치기
            Msg::Escape => {
                if self.error.take().is_some() {
                } else if self.show_settings {
                    self.show_settings = false;
                } else if self.screen == Screen::Typing {
                    self.screen = Screen::Setup;
                    return to_top();
                }
            }
            Msg::Slow(s) => {
                self.slow = s;
                self.play_current();
                return operation::focus(INPUT_ID);
            }
        }
        Task::none()
    }

    /// 새 문장 묶음으로 따라치기 시작.
    fn begin(&mut self, lines: Vec<Line>) -> Task<Msg> {
        self.reset(lines);
        self.screen = Screen::Typing;
        Task::batch([to_top(), self.resume()])
    }

    /// 직접 쓰기 화면을 맨 위부터 보여 주고 편집기에 포커스(보일 때만, resume 과 같은 이유).
    fn show_writing(&self) -> Task<Msg> {
        if !self.visible {
            return to_top();
        }
        Task::batch([to_top(), operation::focus(WRITE_ID)])
    }

    /// 새 문장 묶음을 받고 진행·입력·첨삭 상태를 비운다.
    fn reset(&mut self, lines: Vec<Line>) {
        self.lines = lines;
        self.idx = 0;
        self.typed.clear();
        self.line_start = None;
        (self.total_secs, self.total_correct, self.total_chars) = (0.0, 0, 0);
        self.editor = text_editor::Content::new();
        self.review = None;
        self.draft = None;
        self.review_cards.clear();
        self.error = None;
    }

    /// 첫 첨삭 결과를 오류 유형 기록과 복습 일정에 반영.
    fn record(&mut self, r: &Review) {
        for f in &r.errors {
            if f.kind != Kind::Missing {
                *self
                    .stats
                    .error_kinds
                    .entry(f.kind.key().into())
                    .or_default() += 1;
            }
        }
        self.stats.save();
        if self.review_cards.is_empty() {
            return;
        }
        // ponytail: 구두점만 틀린 줄은 맞은 것으로 친다. 더 엄격하게 하려면 이 조건을 뺀다.
        let failed = |n: usize| {
            r.errors
                .iter()
                .any(|f| f.line == n && f.kind != Kind::Punctuation)
        };
        let t = today();
        for (i, &card) in self.review_cards.iter().enumerate() {
            self.library.grade(card, !failed(i + 1), t);
        }
        self.library.save();
    }

    /// 보일 때만: 따라치기 화면이면 현재 문장을 재생하고 입력칸에 포커스.
    /// 숨어 있을 때 AI·유튜브 결과가 도착하면 여기서 멈추고, 영어 공부 모드로 돌아올 때 앱 셸이 다시 부른다.
    /// (iced 의 focus 연산은 다른 위젯의 포커스를 모두 해제하므로, 숨은 채로 부르면 글쓰기 편집기의 포커스를 뺏는다)
    pub(crate) fn resume(&mut self) -> Task<Msg> {
        if !self.visible || self.screen != Screen::Typing {
            return Task::none();
        }
        self.play_current();
        operation::focus(INPUT_ID)
    }

    /// 유튜브 세트면 현재 문장 구간을 플레이어에서 재생. 플레이어가 없으면 띄우고 브라우저로 연다.
    fn play_current(&mut self) {
        let Some((id, _)) = &self.cur_video else {
            return;
        };
        let Some(line) = self.lines.get(self.idx) else {
            return;
        };
        if self.player.is_none() {
            match Player::start() {
                Ok(p) => {
                    if let Err(e) = youtube::open_in_browser(&p.url()) {
                        self.error = Some(e);
                    }
                    self.player = Some(p);
                }
                Err(e) => {
                    self.error = Some(e);
                    return;
                }
            }
        }
        if let Some(p) = &self.player {
            p.play(id, line.start, line.end, if self.slow { 0.75 } else { 1.0 });
        }
    }

    /// 지금 줄을 마치고 다음 줄로. skip이면 기록(WPM·정확도·틀린 단어)에 넣지 않는다.
    fn finish_line(&mut self, skip: bool) {
        let target = &self.lines[self.idx].en;
        if !skip {
            self.total_secs += self.line_start.map_or(0.0, |t| t.elapsed().as_secs_f64());
            self.total_correct += typing::count_correct(target, &self.typed);
            self.total_chars += target.chars().count();
            for w in typing::missed_words(target, &self.typed) {
                *self.stats.missed.entry(w).or_default() += 1;
            }
            self.stats.lines_typed += 1;
        }
        self.typed.clear();
        self.line_start = None;
        self.idx += 1;
        if self.idx == self.lines.len() {
            self.stats.best_wpm = self.stats.best_wpm.max(self.wpm());
            self.screen = Screen::Writing;
            self.library.add_to_deck(&self.lines, today());
            self.library.save();
            if let Some((id, i)) = &self.cur_video
                && let Some(v) = self.library.video_mut(id)
            {
                v.progress = v.progress.max(i + 1);
                self.library.save();
            }
        }
        self.stats.save();
    }

    fn wpm(&self) -> f64 {
        typing::wpm(self.total_correct, self.total_secs)
    }

    fn accuracy(&self) -> f64 {
        if self.total_chars == 0 {
            100.0
        } else {
            self.total_correct as f64 * 100.0 / self.total_chars as f64
        }
    }

    fn video(&self) -> Option<(&Video, usize)> {
        let (id, i) = self.cur_video.as_ref()?;
        Some((self.library.video(id)?, *i))
    }

    // ---- 화면 ----

    /// 타이틀바·창 제목
    pub(crate) fn title(&self) -> String {
        match (&self.screen, self.video()) {
            (Screen::Setup, _) => "영어 공부".to_string(),
            (Screen::Typing, Some((v, i))) => {
                format!("{} · 세트 {}/{}", v.title, i + 1, v.chunks.len())
            }
            (Screen::Typing, None) => "AI 문장".to_string(),
            (Screen::Writing, _) if !self.review_cards.is_empty() => "복습".to_string(),
            (Screen::Writing, _) => "직접 써보기".to_string(),
        }
    }

    /// 팔레트는 앱 셸이 준다(테마는 하나: 글쓰기 설정의 테마를 두 모드가 같이 쓴다).
    pub(crate) fn view(&self, p: Palette) -> Element<'_, Msg> {
        let title = self.title();
        // 타이틀바 자리: 끌어서 창 이동, 더블클릭으로 확대
        let titlebar = mouse_area(
            container(text(title).size(13).color(p.muted))
                .center(Fill)
                .height(TITLEBAR),
        )
        .on_press(Msg::Drag)
        .on_double_click(Msg::Maximize);

        let body = match self.screen {
            Screen::Setup => self.setup_view(p),
            Screen::Typing => self.typing_view(p),
            Screen::Writing => self.writing_view(p),
        };
        let body = scrollable(
            container(container(body).max_width(720))
                .center_x(Fill)
                .padding(Padding {
                    top: 24.0,
                    bottom: 48.0,
                    left: 32.0,
                    right: 32.0,
                }),
        )
        .id(SCROLL_ID)
        .height(Fill);

        let main = container(column![titlebar, body, self.status_bar(p)])
            .style(move |_| container::Style {
                background: Some(p.bg.into()),
                ..Default::default()
            })
            .width(Fill)
            .height(Fill);
        if !self.show_settings {
            return main.into();
        }
        // 설정: 집중 글쓰기와 같은 자리(오른쪽 위)에 떠 있는 카드
        let settings = container(opaque(
            scrollable(card(p, self.settings_view(p)))
                .height(Fill)
                .width(340),
        ))
        .align_right(Fill)
        .padding(Padding {
            top: TITLEBAR + 8.0,
            right: 16.0,
            bottom: 48.0,
            left: 0.0,
        });
        stack![main, settings].into()
    }

    /// 아래 상태줄: 왼쪽은 알림(작업 중·오류), 오른쪽은 화면별 조용한 정보.
    fn status_bar(&self, p: Palette) -> Element<'_, Msg> {
        let mut bar = row![].spacing(14).align_y(Center);
        if let Some(b) = &self.busy {
            bar = bar.push(text(format!("● {b}")).size(12).color(p.accent));
        }
        if let Some(e) = &self.error {
            bar = bar.push(
                mouse_area(
                    text(format!("⚠ {e}  (클릭해서 닫기)"))
                        .size(12)
                        .color(p.bad),
                )
                .on_press(Msg::DismissError),
            );
        }
        bar = bar.push(space::horizontal());
        let e = self.stats.engine;
        bar = bar.push(
            text(format!("{} · {}", e.name(), e.model_label()))
                .size(12)
                .color(p.muted),
        );
        let info = |s: String| text(s).size(12).color(p.muted);
        match self.screen {
            Screen::Setup => {}
            Screen::Typing => {
                let target = &self.lines[self.idx].en;
                let secs =
                    self.total_secs + self.line_start.map_or(0.0, |t| t.elapsed().as_secs_f64());
                let correct = self.total_correct + typing::count_correct(target, &self.typed);
                bar = bar
                    .push(info(format!("{:.0} WPM", typing::wpm(correct, secs))))
                    .push(info(format!("정확도 {:.0}%", self.accuracy())))
                    .push(info(format!("{} / {}", self.idx + 1, self.lines.len())))
                    .push(
                        button(text("그만하기 · Esc").size(12))
                            .style(quiet(p))
                            .padding([2, 6])
                            .on_press(Msg::Home),
                    );
            }
            Screen::Writing if !self.review_cards.is_empty() => {
                bar = bar.push(info(format!("복습 {}문장", self.lines.len())))
            }
            Screen::Writing => {
                bar = bar
                    .push(info(format!("{:.0} WPM", self.wpm())))
                    .push(info(format!("정확도 {:.0}%", self.accuracy())))
            }
        }
        bar = bar.push(
            button(text("설정").size(12))
                .style(quiet(p))
                .padding([2, 6])
                .on_press(Msg::ToggleSettings),
        );
        container(bar)
            .padding(Padding {
                top: 6.0,
                bottom: 10.0,
                left: 16.0,
                right: 16.0,
            })
            .into()
    }

    /// 처음 화면: 할 일(복습 → 새로 연습) → 보관함 → 기록. 설정은 상태줄의 "설정" 카드로.
    fn setup_view(&self, p: Palette) -> Element<'_, Msg> {
        let (en, ko) = (self.en_font(), self.ko_font());
        let idle = self.busy.is_none();
        let mut page = column![
            column![
                text("영어 공부").size(30).color(p.strong),
                text("따라 치고, 직접 써보며 익히는 영어")
                    .size(14)
                    .color(p.muted),
            ]
            .spacing(4)
        ]
        .spacing(40);

        // 복습: 카드가 생긴 뒤에만. 오늘 할 게 있으면 떠 있는 카드로 눈에 띄게.
        if !self.library.deck.is_empty() {
            let t = today();
            let due = self.library.due(t, usize::MAX).len();
            if due > 0 {
                page = page.push(card(
                    p,
                    row![
                        column![
                            text(format!("오늘 복습할 문장 {due}개"))
                                .size(18)
                                .color(p.strong),
                            text(format!(
                                "며칠 전에 따라친 문장을 한국어만 보고 다시 써봅니다 · 한 번에 {}문장",
                                due.min(REVIEW_MAX)
                            ))
                            .size(13)
                            .color(p.muted),
                        ]
                        .spacing(4)
                        .width(Fill),
                        button(text("복습 시작"))
                            .style(primary(p))
                            .on_press_maybe(idle.then_some(Msg::StartReview))
                            .padding([10, 24]),
                    ]
                    .spacing(12)
                    .align_y(Center),
                ));
            } else {
                let next = self.library.deck.iter().map(|c| c.due).min().unwrap_or(t);
                page = page.push(
                    text(format!(
                        "복습 카드 {}장 · 다음 복습은 {}일 뒤",
                        self.library.deck.len(),
                        next.saturating_sub(t)
                    ))
                    .size(13)
                    .color(p.muted),
                );
            }
        }

        // 유튜브
        page = page.push(section(
            p,
            "유튜브로 연습",
            "영어 자막(자동 생성 포함)을 문장마다 재생 → 멈춤 → 따라 치기. 플레이어는 브라우저에 열립니다.",
            row![
                text_input("https://www.youtube.com/watch?v=…", &self.yt_url)
                    .on_input(Msg::YtUrl)
                    .on_submit_maybe(idle.then_some(Msg::YtLoad))
                    .style(field(p))
                    .padding(10),
                button(text("시작"))
                    .style(primary(p))
                    .on_press_maybe((idle && !self.yt_url.trim().is_empty()).then_some(Msg::YtLoad))
                    .padding([10, 22]),
            ]
            .spacing(8),
        ));

        // AI 문장
        let levels = row(LEVELS
            .iter()
            .map(|l| toggle(p, l, self.level == *l, Msg::Level(l))))
        .spacing(4);
        let genre = row![
            toggle(p, "채팅", self.chat, Msg::Chat(true)),
            toggle(p, "소설", !self.chat, Msg::Chat(false))
        ]
        .spacing(4);
        let topic = text_input("예: ordering coffee, a rainy day…", &self.topic)
            .on_input(Msg::Topic)
            .on_submit_maybe(idle.then_some(Msg::Generate))
            .style(field(p))
            .padding(10);
        page = page.push(section(
            p,
            "AI 문장으로 연습",
            "레벨과 주제를 고르면 AI가 8~12줄을 만들어 줍니다.",
            column![
                labeled(p, "레벨", levels.into()),
                labeled(p, "장르", genre.into()),
                labeled(p, "주제", topic.into()),
                button(text("문장 만들기"))
                    .style(primary(p))
                    .on_press_maybe(idle.then_some(Msg::Generate))
                    .padding([10, 24]),
            ]
            .spacing(12),
        ));

        // 보관함
        let confirming = |d: &Del| self.confirm_delete.as_ref() == Some(d);
        let mut items = Column::new().spacing(2);
        for v in &self.library.videos {
            let n = v.chunks.len();
            let done = v.progress.min(n);
            let del = Del::Video(v.id.clone());
            items = items.push(library_row(
                p,
                format!("▶ {}", v.title),
                format!(
                    "유튜브 · 세트 {done}/{n} 완료 · {}",
                    mmss(v.chunks.last().map_or(0.0, |c| c.end))
                ),
                if done == n {
                    "처음부터"
                } else {
                    "이어하기"
                },
                idle.then(|| Msg::OpenSet(v.id.clone(), resume_set(v))),
                confirming(&del),
                del,
            ));
        }
        for (i, l) in self.library.lessons.iter().enumerate() {
            let del = Del::Lesson(i);
            items = items.push(library_row(
                p,
                l.title.clone(),
                format!("AI 문장 · {}줄", l.lines.len()),
                "열기",
                idle.then_some(Msg::OpenLesson(i)),
                confirming(&del),
                del,
            ));
        }
        let empty = self.library.videos.is_empty() && self.library.lessons.is_empty();
        page = page.push(section(
            p,
            "보관함",
            "만든 문장과 영상은 자동 저장됩니다. 다시 열면 AI를 부르지 않고 바로 시작합니다.",
            if empty {
                Element::from(text("아직 없음").size(14).color(p.muted))
            } else {
                items.into()
            },
        ));

        // 기록: 숫자 · 따라치기 오타 · 첨삭 오류 유형을 한 곳에
        let joined = |v: Vec<String>| {
            if v.is_empty() {
                "아직 없음".to_string()
            } else {
                v.join("   ")
            }
        };
        let mut missed: Vec<_> = self.stats.missed.iter().collect();
        missed.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        let missed = joined(
            missed
                .iter()
                .take(15)
                .map(|(w, n)| format!("{w} ×{n}"))
                .collect(),
        );
        let mut kinds: Vec<_> = self.stats.error_kinds.iter().collect();
        kinds.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        let kinds = joined(
            kinds
                .iter()
                .map(|(k, n)| format!("{} ×{n}", Kind::from_key(k).label()))
                .collect(),
        );
        let stat = |label: &'static str, value: String, font: Font| -> Element<'static, Msg> {
            row![
                text(label).size(13).width(72).color(p.muted),
                text(value).font(font).size(13).color(p.fg)
            ]
            .spacing(12)
            .into()
        };
        let totals = format!(
            "누적 {}줄 · 최고 {:.0} WPM",
            self.stats.lines_typed, self.stats.best_wpm
        );
        page = page.push(section(
            p,
            "기록",
            "",
            column![
                stat("따라치기", totals, UI_FONT),
                stat("틀린 단어", missed, en),
                stat("실수 유형", kinds, ko),
            ]
            .spacing(10),
        ));
        page.into()
    }

    /// 설정 카드: AI 엔진 · 유튜브 · 글꼴. 바꾸면 바로 저장된다.
    fn settings_view(&self, p: Palette) -> Element<'_, Msg> {
        let (en, ko) = (self.en_font(), self.ko_font());
        let idle = self.busy.is_none();
        let small = |t: &'static str| text(t).size(12).color(p.muted);
        let heading = |t: &'static str| text(t).size(14).color(p.strong);

        let engines = Column::with_children([Engine::Claude, Engine::Codex].map(|e| {
            button(text(format!("{} · {}", e.name(), e.model_label())).size(13))
                .style(pill(p, self.stats.engine == e))
                .on_press_maybe(idle.then_some(Msg::Engine(e)))
                .padding([5, 14])
                .into()
        }))
        .spacing(4);
        let cookies = !self.stats.no_yt_cookies;

        // 글꼴 버튼 글자가 그 글꼴로 미리 보인다. 설치 안 된 글꼴은 흐리게, 누를 수 없게
        let choices = |groups: &[(&'static str, Vec<FontItem>)],
                       cur: Font,
                       msg: fn(&'static str) -> Msg|
         -> Element<'_, Msg> {
            Column::with_children(groups.iter().map(|(g, items)| {
                let pills = row(items.iter().map(|&(family, label, ok)| {
                    let face = if ok { Font::with_name(family) } else { UI_FONT };
                    let on = ok && cur == Font::with_name(family);
                    let t = text(label).size(13).font(face);
                    let t = if ok {
                        t
                    } else {
                        t.color(Color { a: 0.4, ..p.fg })
                    };
                    button(t)
                        .style(pill(p, on))
                        .on_press_maybe(ok.then(|| msg(family)))
                        .padding([5, 12])
                        .into()
                }))
                .spacing(4)
                .wrap()
                .vertical_spacing(4);
                column![text(*g).size(11).color(p.muted), pills]
                    .spacing(4)
                    .into()
            }))
            .spacing(10)
            .into()
        };

        column![
            row![
                text("설정").size(16).color(p.strong),
                space::horizontal(),
                button(text("닫기 · Esc").size(12))
                    .style(quiet(p))
                    .padding([2, 6])
                    .on_press(Msg::ToggleSettings),
            ]
            .align_y(Center),
            column![
                heading("AI 엔진"),
                small("문장 만들기 · 자막 정리 · 첨삭에 씁니다. 터미널에서 미리 로그인해 두세요."),
                engines,
            ]
            .spacing(6),
            column![
                heading("유튜브"),
                toggle(p, "Chrome 로그인 쿠키 사용", cookies, Msg::Cookies(!cookies)),
                small("YouTube 요청 제한(429)을 피합니다. 처음 한 번 키체인 허용 창이 뜹니다."),
            ]
            .spacing(6),
            column![
                heading("영문 글꼴 — 따라치기 · 입력 · 작문"),
                choices(&self.fonts.0, en, Msg::EnFont),
                text("The quick brown fox jumps over the lazy dog.")
                    .font(en)
                    .size(16)
                    .color(p.strong),
            ]
            .spacing(8),
            column![
                heading("한글 글꼴 — 뜻 · 첨삭 설명"),
                choices(&self.fonts.1, ko, Msg::KoFont),
                text("다람쥐 헌 쳇바퀴에 타고파")
                    .font(ko)
                    .size(16)
                    .color(p.strong),
                small("흐린 글꼴은 설치하면 고를 수 있습니다. 명조↔세리프, 고딕↔산세리프끼리 짝지으면 잘 어울립니다."),
            ]
            .spacing(8),
        ]
        .spacing(22)
        .into()
    }

    fn typing_view(&self, p: Palette) -> Element<'_, Msg> {
        let (en, ko) = (self.en_font(), self.ko_font());
        let line = &self.lines[self.idx];
        let target = &line.en;
        let video = self.video();

        let progress = progress_bar(0.0..=self.lines.len() as f32, self.idx as f32)
            .girth(2)
            .style(move |_| progress_bar::Style {
                background: p.hair().into(),
                bar: p.accent.into(),
                border: Border::default(),
            });

        // 포커스 모드: 끝낸 줄은 흐리게, 지금 줄만 또렷하게
        let done = Column::with_children(self.lines[..self.idx].iter().map(|l| {
            text(format!("{}{}", l.who(), l.en))
                .font(en)
                .size(15)
                .color(p.muted)
                .into()
        }))
        .spacing(6);

        let states = typing::char_states(target, &self.typed);
        let next = states.iter().position(|s| matches!(s, CharState::Pending));
        let spans: Vec<_> = target
            .chars()
            .zip(states)
            .enumerate()
            .map(|(i, (c, s))| {
                let (ch, color) = match s {
                    CharState::Correct => (c, p.strong),
                    CharState::Wrong if c == ' ' => ('_', p.bad),
                    CharState::Wrong => (c, p.bad),
                    CharState::Pending if Some(i) == next => (c, p.accent),
                    CharState::Pending => (c, p.muted),
                };
                span::<(), Font>(ch.to_string())
                    .color(color)
                    .underline(Some(i) == next)
            })
            .collect();

        // 입력칸과 목표 문장의 글꼴·크기·왼쪽 여백을 맞춰 글자가 위아래로 줄 서게 한다
        let pad = 14.0;
        let input = text_input("여기에 따라 치세요", &self.typed)
            .id(INPUT_ID)
            .on_input(Msg::Typed)
            .on_submit(Msg::Submit)
            .font(en)
            .size(24)
            .style(field(p))
            .padding(Padding {
                top: 12.0,
                bottom: 12.0,
                left: pad,
                right: pad,
            });

        let mut current = column![
            container(
                row![
                    text(line.who()).font(en).size(24).color(p.accent),
                    rich_text(spans).font(en).size(24)
                ]
                .wrap()
            )
            .padding(Padding {
                left: pad,
                ..Padding::ZERO
            }),
            container(text(&line.ko).font(ko).size(15).color(p.fg)).padding(Padding {
                left: pad,
                ..Padding::ZERO
            }),
            input,
        ]
        .spacing(12);

        let mut controls = row![].spacing(6).align_y(Center);
        if video.is_some() {
            controls = controls
                .push(
                    button(text("↻ 다시 듣기").size(13))
                        .style(quiet(p))
                        .on_press(Msg::Replay)
                        .padding([4, 10]),
                )
                .push(toggle(p, "0.75배속", self.slow, Msg::Slow(!self.slow)))
                .push(
                    button(text("플레이어 열기").size(13))
                        .style(quiet(p))
                        .on_press(Msg::OpenPlayer)
                        .padding([4, 10]),
                );
        }
        controls = controls
            .push(
                button(text("건너뛰기 →").size(13))
                    .style(quiet(p))
                    .on_press(Msg::Skip)
                    .padding([4, 10]),
            )
            .push(space::horizontal());
        if video.is_some() {
            controls = controls.push(
                text(format!("{} – {}", mmss(line.start), mmss(line.end)))
                    .size(12)
                    .color(p.muted),
            );
        }
        current = current.push(controls);
        let hint = if video.is_some() {
            "Enter 다음 문장 · 빈 칸에서 Enter 다시 듣기 · Tab 건너뛰기 · Esc 그만하기"
        } else {
            "줄 끝까지 친 뒤 Enter · Tab 건너뛰기 · Esc 그만하기"
        };
        current = current.push(text(hint).size(12).color(p.muted));

        column![progress, done, current].spacing(28).into()
    }

    /// 직접 쓰기: 처음 쓰기 → 첨삭 → (교정문 가리고) 다시 쓰기 → 다시 첨삭 + 비교.
    fn writing_view(&self, p: Palette) -> Element<'_, Msg> {
        let (en, ko) = (self.en_font(), self.ko_font());
        let idle = self.busy.is_none();
        let reviewing = !self.review_cards.is_empty();
        let summary = if reviewing {
            column![
                text("복습").size(26).color(p.strong),
                text(format!(
                    "며칠 전에 따라친 {}문장입니다. 한국어만 보고 영어로 써보세요.",
                    self.lines.len()
                ))
                .size(14)
                .color(p.muted),
            ]
        } else {
            column![
                text("따라치기 완료").size(26).color(p.strong),
                text(format!(
                    "{:.0} WPM · 정확도 {:.0}%",
                    self.wpm(),
                    self.accuracy()
                ))
                .size(14)
                .color(p.muted),
            ]
        }
        .spacing(4);

        let prompts = Column::with_children(self.lines.iter().enumerate().map(|(i, l)| {
            row![
                text(format!("{:>2}", i + 1))
                    .font(en)
                    .size(13)
                    .color(p.muted),
                text(format!("{}{}", l.who(), l.ko))
                    .font(ko)
                    .size(15)
                    .color(p.fg),
            ]
            .spacing(12)
            .into()
        }))
        .spacing(6);

        let rewriting = self.draft.is_some();
        let editor = text_editor(&self.editor)
            .id(WRITE_ID)
            .placeholder(if rewriting {
                "교정문을 떠올리며 다시 써보세요…"
            } else {
                "영어로 써보세요…"
            })
            .on_action(Msg::Edit)
            .font(en)
            .size(16)
            .line_height(1.6)
            .height(220)
            .padding(16)
            .style(move |_, st| text_editor::Style {
                background: p.panel.into(),
                border: Border::default().rounded(10).width(1).color(
                    if matches!(st, text_editor::Status::Focused { .. }) {
                        p.accent
                    } else {
                        p.hair()
                    },
                ),
                placeholder: p.muted,
                value: p.strong,
                selection: Color { a: 0.3, ..p.accent },
            });

        let can = idle && !self.editor.text().trim().is_empty();
        let mut actions = row![].spacing(8).align_y(Center);
        match (&self.review, rewriting) {
            // 첨삭 전: 처음 쓰기든 다시 쓰기든 첨삭 받기
            (None, _) => {
                actions = actions.push(
                    button(text(if rewriting {
                        "다시 첨삭 받기"
                    } else {
                        "첨삭 받기"
                    }))
                    .style(primary(p))
                    .on_press_maybe(can.then_some(Msg::AskReview))
                    .padding([10, 24]),
                )
            }
            // 처음 쓴 글의 첨삭을 받았으면: 가리고 다시 쓰기
            (Some(_), false) => {
                actions = actions.push(
                    button(text("교정문 가리고 다시 쓰기"))
                        .style(primary(p))
                        .on_press_maybe(idle.then_some(Msg::StartRewrite))
                        .padding([10, 24]),
                )
            }
            (Some(_), true) => {}
        }
        if let Some((v, i)) = self.video()
            && i + 1 < v.chunks.len()
        {
            actions = actions.push(
                button(text(format!("다음 세트 ({}/{}) →", i + 2, v.chunks.len())))
                    .style(pill(p, false))
                    .on_press_maybe(idle.then(|| Msg::OpenSet(v.id.clone(), i + 1)))
                    .padding([10, 20]),
            );
        }
        actions = actions.push(
            button(text("처음 화면"))
                .style(quiet(p))
                .on_press(Msg::Home)
                .padding([10, 16]),
        );

        let (title, desc) = if rewriting {
            (
                "다시 써보세요",
                "교정문은 가려 두었습니다. 고친 곳을 떠올리며 한국어만 보고 다시 써보세요.",
            )
        } else if reviewing {
            (
                "기억나는 대로 써보세요",
                "원문과 똑같지 않아도 됩니다. 첨삭에서 고칠 곳이 나온 문장은 곧 다시 나옵니다.",
            )
        } else {
            (
                "이제 직접 써보세요",
                "한국어를 보고 방금 친 영어를 떠올려 써보세요. 똑같지 않아도 됩니다. (건너뛰어도 됩니다)",
            )
        };
        // 첨삭이 오면 결과를 맨 위에(Reviewed 가 스크롤도 맨 위로 올린다), 쓰는 칸은 그 아래
        let mut page = column![summary].spacing(40);

        // 다시 쓴 글의 첨삭까지 받았으면 처음 글과 나란히 비교
        if let (Some((first, first_r)), Some(r)) = (&self.draft, &self.review) {
            let count = |r: &Review| r.errors.iter().filter(|f| f.kind != Kind::Missing).count();
            let label = |t: String| text(t).size(12).color(p.muted);
            page = page.push(card(
                p,
                column![
                    text(format!("고칠 곳 {}개 → {}개", count(first_r), count(r)))
                        .size(18)
                        .color(p.strong),
                    label("처음 쓴 글".into()),
                    text(first).font(en).color(p.fg),
                    label("다시 쓴 글".into()),
                    text(self.editor.text()).font(en).color(p.fg),
                    label("교정문 (처음 쓴 글)".into()),
                    text(&first_r.corrected).font(en).color(p.strong),
                ]
                .spacing(10),
            ));
        }
        if let Some(r) = &self.review {
            page = page.push(self.review_card(p, r));
        }
        page = page.push(section(
            p,
            title,
            desc,
            column![prompts, editor, actions].spacing(16),
        ));
        page.into()
    }

    /// 첨삭 카드. 고친 곳은 이번에 집중할 유형을 먼저, 나머지는 흐리게.
    fn review_card<'a>(&'a self, p: Palette, r: &'a Review) -> Element<'a, Msg> {
        let (en, ko) = (self.en_font(), self.ko_font());
        let label = |t: String| text(t).size(12).color(p.muted);
        let focus = focus_kinds(r, &self.stats.error_kinds);
        let fix_view = |f: &'a claude::Fix, strong: bool| -> Element<'a, Msg> {
            let dim = |c: Color| if strong { c } else { Color { a: 0.6, ..c } };
            column![
                rich_text![
                    span::<(), Font>(format!("{}  ", f.kind.label()))
                        .font(ko)
                        .size(12)
                        .color(p.muted),
                    span(&f.original)
                        .font(en)
                        .color(dim(p.bad))
                        .strikethrough(true),
                    span("  →  ").color(p.muted),
                    span(&f.fix).font(en).color(dim(p.ok)),
                ],
                text(&f.why_ko).font(ko).size(13).color(p.muted),
            ]
            .spacing(2)
            .into()
        };
        let (main, rest): (Vec<_>, Vec<_>) = r.errors.iter().partition(|f| focus.contains(&f.kind));
        let mut col = column![
            label("교정문".into()),
            text(&r.corrected).font(en).color(p.strong)
        ]
        .spacing(12);
        if !focus.is_empty() {
            let names: Vec<_> = focus.iter().map(|k| k.label()).collect();
            col = col
                .push(label(format!("이번에 집중할 것 — {}", names.join(" · "))))
                .push(
                    Column::with_children(main.into_iter().map(|f| fix_view(f, true))).spacing(12),
                );
        }
        if !rest.is_empty() {
            col = col
                .push(label(if focus.is_empty() {
                    "고친 곳".into()
                } else {
                    "그 밖에 고친 곳".into()
                }))
                .push(
                    Column::with_children(rest.into_iter().map(|f| fix_view(f, false))).spacing(12),
                );
        }
        let model = Column::with_children(self.lines.iter().map(|l| {
            text(format!("{}{}", l.who(), l.en))
                .font(en)
                .size(14)
                .color(p.fg)
                .into()
        }))
        .spacing(4);
        col = col
            .push(label("총평".into()))
            .push(text(&r.feedback_ko).font(ko).color(p.fg))
            .push(label("모범 답안 (원문)".into()))
            .push(model);
        card(p, col)
    }
}

/// 이번 첨삭에서 집중할 오류 유형 (최대 2개): 이번에 많이 나온 순, 같으면 누적이 많은 순.
/// 빠뜨림·구두점·기타는 문법 습관이 아니라서 빼고, 2번 이상 나온 유형만 본다.
// ponytail: 하나뿐인 실수까지 다 강조하면 집중이 안 된다. 1번짜리도 보고 싶으면 n >= 2 조건을 뺀다.
fn focus_kinds(r: &Review, history: &HashMap<String, u32>) -> Vec<Kind> {
    let mut n: HashMap<Kind, usize> = HashMap::new();
    for f in &r.errors {
        if !matches!(f.kind, Kind::Missing | Kind::Punctuation | Kind::Other) {
            *n.entry(f.kind).or_default() += 1;
        }
    }
    let past = |k: &Kind| history.get(k.key()).copied().unwrap_or(0);
    let mut v: Vec<_> = n.into_iter().filter(|&(_, c)| c >= 2).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(past(&b.0).cmp(&past(&a.0))));
    v.into_iter().take(2).map(|(k, _)| k).collect()
}

fn to_top() -> Task<Msg> {
    operation::snap_to(SCROLL_ID, operation::RelativeOffset::START)
}

/// 이어할 세트: 다 끝났으면 처음부터.
fn resume_set(v: &Video) -> usize {
    if v.progress >= v.chunks.len() {
        0
    } else {
        v.progress
    }
}

// ---- 스타일 (iamWriter 방식: 테두리 대신 글자 크기·색·여백으로 위계) ----

type ButtonStyle = Box<dyn Fn(&iced::Theme, button::Status) -> button::Style>;

fn primary(p: Palette) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, st| button::Style {
        background: Some(
            match st {
                button::Status::Hovered | button::Status::Pressed => Color {
                    a: 0.85,
                    ..p.accent
                },
                button::Status::Disabled => Color {
                    a: 0.35,
                    ..p.accent
                },
                button::Status::Active => p.accent,
            }
            .into(),
        ),
        text_color: p.bg,
        border: Border::default().rounded(8),
        ..Default::default()
    }
}

/// 켜고 끄는 알약 버튼.
fn pill(p: Palette, on: bool) -> ButtonStyle {
    Box::new(move |_, st| {
        let hover = matches!(st, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: Some(
                if on {
                    p.accent
                } else {
                    Color {
                        a: if hover { 0.14 } else { 0.07 },
                        ..p.fg
                    }
                }
                .into(),
            ),
            text_color: if on { p.bg } else { p.fg },
            border: Border::default().rounded(14),
            ..Default::default()
        }
    })
}

/// 글자만 있는 버튼. 마우스를 올리면 옅은 강조색 바탕.
fn quiet(p: Palette) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, st| button::Style {
        background: matches!(st, button::Status::Hovered | button::Status::Pressed)
            .then(|| Color { a: 0.1, ..p.accent }.into()),
        text_color: p.fg,
        border: Border::default().rounded(6),
        ..Default::default()
    }
}

fn field(p: Palette) -> impl Fn(&iced::Theme, text_input::Status) -> text_input::Style {
    move |_, st| text_input::Style {
        background: p.panel.into(),
        border: Border::default().rounded(8).width(1).color(
            if matches!(st, text_input::Status::Focused { .. }) {
                p.accent
            } else {
                p.hair()
            },
        ),
        icon: p.muted,
        placeholder: p.muted,
        value: p.strong,
        selection: Color { a: 0.3, ..p.accent },
    }
}

// ---- 작은 UI 조각 ----

/// 떠 있는 카드: 패널 바탕, 가는 테두리, 부드러운 그림자.
fn card<'a>(p: Palette, content: impl Into<Element<'a, Msg>>) -> Element<'a, Msg> {
    container(content)
        .padding(22)
        .width(Fill)
        .style(move |_| container::Style {
            background: Some(p.panel.into()),
            border: Border::default()
                .rounded(12)
                .width(1)
                .color(Color { a: 0.08, ..p.fg }),
            shadow: Shadow {
                color: Color {
                    a: 0.12,
                    ..Color::BLACK
                },
                offset: Vector::new(0.0, 6.0),
                blur_radius: 20.0,
            },
            ..Default::default()
        })
        .into()
}

/// 제목 + 설명 + 내용. 상자 없이 여백으로 구분한다.
fn section<'a>(
    p: Palette,
    title: &'a str,
    desc: &'a str,
    content: impl Into<Element<'a, Msg>>,
) -> Element<'a, Msg> {
    let mut head = column![text(title).size(18).color(p.strong)].spacing(4);
    if !desc.is_empty() {
        head = head.push(text(desc).size(13).color(p.muted));
    }
    column![head, content.into()].spacing(14).into()
}

/// 보관함 한 줄. 삭제는 두 번 눌러야 한다(영상은 자막을 다시 받아야 해서 되돌리기 어렵다).
fn library_row(
    p: Palette,
    title: String,
    meta: String,
    open: &str,
    on_open: Option<Msg>,
    confirming: bool,
    del: Del,
) -> Element<'_, Msg> {
    let r = row![
        column![
            text(title).size(15).color(p.strong),
            text(meta).size(12).color(p.muted)
        ]
        .spacing(2)
        .width(Fill),
        button(text(open).size(13))
            .style(pill(p, false))
            .on_press_maybe(on_open)
            .padding([5, 14]),
    ]
    .spacing(6)
    .align_y(Center)
    .padding([8, 0]);
    let r = if confirming {
        let delete = match del {
            Del::Video(id) => Msg::DeleteVideo(id),
            Del::Lesson(i) => Msg::DeleteLesson(i),
        };
        r.push(
            button(text("정말 삭제").size(12).color(p.bad))
                .style(quiet(p))
                .on_press(delete)
                .padding([5, 8]),
        )
        .push(
            button(text("취소").size(12).color(p.muted))
                .style(quiet(p))
                .on_press(Msg::CancelDelete)
                .padding([5, 8]),
        )
    } else {
        r.push(
            button(text("삭제").size(12).color(p.muted))
                .style(quiet(p))
                .on_press(Msg::AskDelete(del))
                .padding([5, 8]),
        )
    };
    r.into()
}

fn toggle(p: Palette, label: &str, on: bool, msg: Msg) -> Element<'_, Msg> {
    button(text(label).size(13))
        .style(pill(p, on))
        .on_press(msg)
        .padding([5, 14])
        .into()
}

fn labeled<'a>(p: Palette, label: &'a str, content: Element<'a, Msg>) -> Element<'a, Msg> {
    row![text(label).size(13).width(48).color(p.muted), content]
        .spacing(12)
        .align_y(Center)
        .into()
}

#[test]
fn default_fonts_are_found() {
    let (en, ko) = (font_groups(EN_FONTS), font_groups(KO_FONTS));
    let installed = |g: &Vec<(&str, Vec<FontItem>)>, n: &str| {
        g.iter().flat_map(|(_, v)| v).any(|i| i.0 == n && i.2)
    };
    assert!(installed(&en, EN_DEFAULT) && installed(&ko, KO_DEFAULT));
    assert_eq!(
        pick_font(&en, &Some("없는 글꼴".into()), EN_DEFAULT),
        Font::with_name(EN_DEFAULT)
    );
    // 같은 라벨은 하나로 묶인다
    assert_eq!(
        ko.iter()
            .flat_map(|(_, v)| v)
            .filter(|i| i.1 == "본고딕")
            .count(),
        1
    );
}

#[test]
fn hidden_practice_neither_plays_video_nor_steals_focus() {
    // 집중 글쓰기 모드에 있는 동안 유튜브 세트 변환이 끝난 상황. cur_video 가 있어야 play_current 가 실제로 동작하려 한다.
    let mut p = Practice::new();
    assert!(!p.visible);
    p.cur_video = Some(("jNQXAC9IVRw".into(), 0));
    let line = Line {
        speaker: String::new(),
        en: "Hi there.".into(),
        ko: "안녕.".into(),
        start: 1.0,
        end: 2.0,
    };
    let _ = p.begin(vec![line]);
    assert!(p.screen == Screen::Typing, "결과는 받아 둔다");
    assert!(
        p.player.is_none(),
        "숨은 채로 플레이어 서버를 띄우거나 브라우저를 열면 안 된다"
    );
    // 보이게 되는 쪽(resume)은 실제로 서버를 띄우고 브라우저를 열기 때문에 단위 테스트하지 않는다(수동 점검)
}

#[test]
fn focus_picks_repeated_kinds_and_breaks_ties_by_history() {
    let fix = |kind| claude::Fix {
        original: "x".into(),
        fix: "y".into(),
        why_ko: String::new(),
        kind,
        line: 1,
    };
    let r = Review {
        corrected: String::new(),
        feedback_ko: String::new(),
        errors: vec![
            fix(Kind::Tense),
            fix(Kind::Tense),
            fix(Kind::Article),
            fix(Kind::Article),
            fix(Kind::Preposition),
            fix(Kind::Preposition),
            fix(Kind::Spelling),
            fix(Kind::Missing),
            fix(Kind::Missing),
        ],
    };
    let history = HashMap::from([("preposition".to_string(), 9), ("article".to_string(), 1)]);
    // 셋 다 2번 → 누적 많은 전치사가 먼저, 한 번뿐인 철자·빠뜨림은 제외
    assert_eq!(
        focus_kinds(&r, &history),
        vec![Kind::Preposition, Kind::Article]
    );
}

#[test]
fn rewrite_keeps_first_draft_and_counts_errors_once() {
    let mut p = Practice::new();
    p.reset(vec![Line {
        speaker: String::new(),
        en: "I went home.".into(),
        ko: "나는 집에 갔다.".into(),
        start: 0.0,
        end: 0.0,
    }]);
    p.screen = Screen::Writing;
    let review = || Review {
        corrected: "I went home.".into(),
        feedback_ko: String::new(),
        errors: vec![claude::Fix {
            original: "go".into(),
            fix: "went".into(),
            why_ko: String::new(),
            kind: Kind::Tense,
            line: 1,
        }],
    };
    let before = p.stats.error_kinds.get("tense").copied().unwrap_or(0);
    let _ = p.update(Msg::Reviewed(Ok(review())));
    let _ = p.update(Msg::StartRewrite);
    assert!(p.review.is_none() && p.draft.is_some(), "교정문은 가린다");
    let _ = p.update(Msg::Reviewed(Ok(review())));
    assert_eq!(
        p.stats.error_kinds["tense"],
        before + 1,
        "다시 쓴 글은 세지 않는다"
    );
}

#[test]
fn escape_closes_topmost_first_and_delete_needs_confirm() {
    let mut p = Practice::new();
    p.screen = Screen::Typing;
    p.lines = vec![Line {
        speaker: String::new(),
        en: "Hi.".into(),
        ko: "안녕.".into(),
        start: 0.0,
        end: 0.0,
    }];
    p.show_settings = true;
    p.error = Some("x".into());
    let _ = p.update(Msg::Escape);
    assert!(p.error.is_none() && p.show_settings && p.screen == Screen::Typing);
    let _ = p.update(Msg::Escape);
    assert!(!p.show_settings && p.screen == Screen::Typing);
    let _ = p.update(Msg::Escape);
    assert!(p.screen == Screen::Setup);

    // 삭제: 한 번 누르면 묻기만 하고, 취소하면 그대로
    p.library.lessons.push(Lesson {
        title: "t".into(),
        lines: vec![],
    });
    let n = p.library.lessons.len();
    let _ = p.update(Msg::AskDelete(Del::Lesson(n - 1)));
    assert_eq!(p.library.lessons.len(), n);
    let _ = p.update(Msg::CancelDelete);
    assert!(p.confirm_delete.is_none() && p.library.lessons.len() == n);
}
