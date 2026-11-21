// 영어 공부 모드 — typingEng(따라치기 + 영작 첨삭) 이식
// 원본: github.com/1ncarnati0n/typingEng. 이식하며 달라진 곳은 docs/TYPINGENG_PORT.md 5장,
// 이식 뒤 새로 넣은 기능(다시 쓰기 · 오류 유형 · 복습)은 같은 문서 11장에 있다.
mod claude;
mod player;
mod store;
mod typing;
mod youtube;

use crate::theme::{Palette, TITLEBAR, UI_FONT};
use claude::{Engine, Genre, KeySource, Kind, Line, Review};
use iced::widget::{
    Column, button, column, container, mouse_area, operation, progress_bar, rich_text, row,
    scrollable, space, span, text, text_editor, text_input,
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

/// 처음 화면 왼쪽 사이드바의 기능 목록
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Section {
    Review,
    YouTube,
    Ai,
    Write,
    Library,
    Records,
}

impl Section {
    const ALL: [Section; 6] = [
        Section::Review,
        Section::YouTube,
        Section::Ai,
        Section::Write,
        Section::Library,
        Section::Records,
    ];
    fn label(self) -> &'static str {
        match self {
            Section::Review => "복습",
            Section::YouTube => "영상",
            Section::Ai => "AI 문장",
            Section::Write => "직접 쓰기",
            Section::Library => "보관함",
            Section::Records => "기록",
        }
    }
}

pub(crate) struct Practice {
    stats: Stats,
    library: Library,
    screen: Screen,
    /// 처음 화면에서 고른 기능
    section: Section,
    /// 지금 문장이 보관함의 AI 문장이면 그 장르(직접 쓰기 안내가 달라진다). 유튜브 · 복습이면 None
    lesson_genre: Option<Genre>,
    level: &'static str,
    genre: Genre,
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
    /// 설정 창 API 키 입력칸 (저장하면 비운다. 키는 키체인에만 둔다)
    api_key: String,
    /// API 키가 어디 있는지. 키체인 조회는 프로세스를 띄우므로 바뀔 때만 다시 읽는다.
    key_source: KeySource,
    /// 마지막 연결 확인 결과
    checked: Option<Result<String, String>>,
    /// 실시간 모델 목록: None 아직 안 받음 · Some(Ok(0)) 받는 중 · Some(Ok(n)) n개 · Some(Err) 실패
    models: Option<Result<usize, String>>,
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
    /// 자막과 함께 보고 있는 영상 ID
    watching: Option<String>,
    /// 보기용 자막을 뒤에서 번역 중인 세트 (영상 ID, 세트 번호). busy 와 달리 다른 버튼을 막지 않는다
    converting: Option<(String, usize)>,
    /// 따라치기로 열려고 번역을 기다리는 세트. 뒤에서 이미 번역 중이면 그 결과를 받아 연다(두 번 부르지 않게)
    opening: Option<(String, usize)>,
    /// 숨어 있을 때 보기가 정해졌으면, 보일 때 플레이어를 연다(resume)
    watch_pending: bool,
    /// 설치된 추천 글꼴 (영문, 한글)
    fonts: (FontGroups, FontGroups),
    /// 영어 공부 모드가 화면에 보이는지. 숨어 있을 땐 포커스·재생 같은 부수효과를 미룬다(resume).
    pub(crate) visible: bool,
}

#[derive(Debug, Clone)]
pub(crate) enum Msg {
    Level(&'static str),
    Genre(Genre),
    Topic(String),
    Generate,
    Generated(Result<Vec<Line>, String>),
    Typed(String),
    Submit,
    Edit(text_editor::Action),
    AskReview,
    Reviewed(Result<Review, String>),
    StartRewrite,
    /// 복습 시작. true 면 미리 복습(기한 전 카드도, 가까운 순)
    StartReview(bool),
    ToggleSettings,
    AskDelete(Del),
    CancelDelete,
    Home,
    YtUrl(String),
    YtLoad,
    YtLoaded(Result<youtube::Transcript, String>),
    OpenSet(String, usize),
    /// 영상을 자막과 함께 보기 (보관함에 없으면 자막부터 받는다)
    Watch(String),
    WatchSet(String, usize, Result<Vec<Line>, String>),
    StopWatch,
    SetReady(String, usize, Result<Vec<Line>, String>),
    OpenLesson(usize),
    /// 보관함 문장으로 따라치기 없이 바로 영작
    WriteLesson(usize),
    Section(Section),
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
    Model(String),
    ApiKey(String),
    SaveKey,
    DeleteKey,
    CheckAi,
    RefreshModels,
    ModelsLoaded(Result<usize, String>),
    Checked(Result<String, String>),
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
        let library = Library::load();
        // 오늘 복습할 게 있으면 복습부터, 없으면 새 문장 만들기부터
        let section = if library.due(today(), 1).is_empty() {
            Section::Ai
        } else {
            Section::Review
        };
        Self {
            stats: Stats::load(),
            library,
            screen: Screen::Setup,
            section,
            lesson_genre: None,
            level: "A2",
            genre: Genre::Chat,
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
            api_key: String::new(),
            // 테스트는 실제 키체인을 읽지 않는다
            key_source: if cfg!(test) {
                KeySource::Missing
            } else {
                claude::key_source()
            },
            checked: None,
            models: None,
            confirm_delete: None,
            busy: None,
            error: None,
            yt_url: String::new(),
            cur_video: None,
            player: None,
            slow: false,
            watching: None,
            converting: None,
            opening: None,
            watch_pending: false,
            fonts: (font_groups(EN_FONTS), font_groups(KO_FONTS)),
            visible: false, // 앱은 집중 글쓰기 모드로 시작한다
        }
    }

    /// 기록·설정 저장. 실패하면 상태줄 오류로 알린다(조용히 잃지 않게).
    fn save_stats(&mut self) {
        if let Err(e) = self.stats.save() {
            self.error = Some(e);
        }
    }

    /// 보관함 저장. 실패하면 상태줄 오류로 알린다.
    fn save_library(&mut self) {
        if let Err(e) = self.library.save() {
            self.error = Some(e);
        }
    }

    /// "Codex(gpt-6.1-terra)가 … 중…" 같은 작업 안내.
    fn working(&self, what: &str) -> String {
        format!("{} — {what}…", self.stats.ai().label())
    }

    fn en_font(&self) -> Font {
        pick_font(&self.fonts.0, &self.stats.en_font, EN_DEFAULT)
    }

    fn ko_font(&self) -> Font {
        pick_font(&self.fonts.1, &self.stats.ko_font, KO_DEFAULT)
    }

    /// Esc·Tab 만. ⌘, 설정 · 창 닫기 · 시스템 테마 구독은 앱 셸(main.rs)이 한다.
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
            _ => None,
        })
    }

    pub(crate) fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Level(l) => self.level = l,
            Msg::Genre(g) => {
                // 주제가 이전 장르의 예시 그대로면 새 장르의 첫 예시로 (직접 쓴 주제는 그대로)
                if self.genre.examples().contains(&self.topic.as_str()) {
                    self.topic = g.examples()[0].into();
                }
                self.genre = g;
            }
            Msg::Topic(t) => self.topic = t,
            Msg::Generate => {
                self.busy = Some(self.working("문장을 쓰는 중"));
                self.error = None;
                let (ai, level, genre, topic) =
                    (self.stats.ai(), self.level, self.genre, self.topic.clone());
                return Task::perform(
                    off_thread(move || claude::generate_lines(&ai, level, genre, &topic)),
                    Msg::Generated,
                );
            }
            Msg::Generated(res) => {
                self.busy = None;
                match res {
                    Ok(lines) => {
                        let genre = self.genre.label();
                        let title = format!("{} · {genre} · {}", self.level, self.topic.trim());
                        self.library.lessons.insert(
                            0,
                            Lesson {
                                title,
                                lines: lines.clone(),
                                genre: Some(self.genre),
                            },
                        );
                        self.save_library();
                        self.cur_video = None;
                        let task = self.begin(lines);
                        self.lesson_genre = Some(self.genre);
                        return task;
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
                self.save_stats();
                self.checked = None;
                return self.load_models();
            }
            Msg::RefreshModels => {
                self.models = None;
                return self.load_models();
            }
            Msg::ModelsLoaded(r) => self.models = Some(r),
            Msg::Model(m) => {
                self.stats.models.insert(self.stats.engine, m);
                self.save_stats();
                self.checked = None;
            }
            Msg::ApiKey(k) => self.api_key = k,
            Msg::SaveKey => match claude::save_key(&self.api_key) {
                Ok(()) => {
                    self.api_key.clear();
                    self.key_source = claude::key_source();
                    self.checked = None;
                    self.models = None;
                    return self.load_models();
                }
                Err(e) => self.checked = Some(Err(e)),
            },
            Msg::DeleteKey => {
                claude::delete_key();
                self.key_source = claude::key_source();
                self.checked = None;
            }
            Msg::CheckAi => {
                self.busy = Some(self.working("연결 확인 중"));
                self.checked = None;
                let ai = self.stats.ai();
                return Task::perform(off_thread(move || claude::check(&ai)), Msg::Checked);
            }
            Msg::Checked(r) => {
                self.busy = None;
                self.checked = Some(r);
            }
            Msg::EnFont(f) => {
                self.stats.en_font = Some(f.into());
                self.save_stats();
            }
            Msg::KoFont(f) => {
                self.stats.ko_font = Some(f.into());
                self.save_stats();
            }
            Msg::Edit(a) => self.editor.perform(a),
            Msg::AskReview => {
                self.busy = Some(self.working("첨삭하는 중"));
                self.error = None;
                let (ai, lines, written) =
                    (self.stats.ai(), self.lines.clone(), self.editor.text());
                return Task::perform(
                    off_thread(move || claude::review_writing(&ai, &lines, &written)),
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
            Msg::StartReview(early) => {
                // 미리 복습: 기한과 상관없이 가까운 순. 맞히면 오늘부터 다음 간격이 시작된다
                let until = if early { u32::MAX } else { today() };
                let cards = self.library.due(until, REVIEW_MAX);
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
                    let msg = Msg::Watch(v.id.clone());
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
                    self.save_library();
                    self.yt_url.clear();
                    self.busy = None;
                    return self.update(Msg::Watch(t.id));
                }
                Err(e) => {
                    self.busy = None;
                    self.error = Some(e);
                }
            },
            Msg::Watch(id) => {
                if self.library.video(&id).is_none() {
                    self.yt_url = youtube::watch_url(&id);
                    return self.update(Msg::YtLoad);
                }
                self.watching = Some(id);
                self.error = None;
                if !self.visible {
                    self.watch_pending = true;
                    return Task::none();
                }
                return self.start_watch();
            }
            Msg::WatchSet(id, i, res) => {
                self.converting = None;
                let job = (id.clone(), i);
                let waiting = self.opening.as_ref() == Some(&job);
                if waiting {
                    self.opening = None;
                    self.busy = None;
                }
                match res {
                    Ok(lines) => {
                        if let Some(set) =
                            self.library.video_mut(&id).and_then(|v| v.sets.get_mut(i))
                        {
                            *set = Some(lines.clone());
                        }
                        self.save_library();
                        if self.watching.as_ref() == Some(&id) {
                            self.push_subs();
                        }
                        // 따라치기가 이 세트를 기다리고 있었으면 바로 연다
                        let open = if waiting {
                            self.cur_video = Some(job);
                            self.begin(lines)
                        } else {
                            Task::none()
                        };
                        // 지금 보는 영상(번역하던 영상과 달라도)의 다음 세트로
                        return Task::batch([open, self.convert_next()]);
                    }
                    Err(e) => {
                        self.error = Some(e);
                        // 보던 영상의 번역이 실패하면 멈춘다(같은 실패를 되풀이하지 않게. 다시 보기로 이어서).
                        // 다른 영상으로 바꾼 뒤에 옛 영상이 실패한 거면 지금 영상은 시작한다
                        if self.watching.as_ref() != Some(&id) {
                            return self.convert_next();
                        }
                    }
                }
            }
            Msg::StopWatch => self.watching = None,
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
                let job = (id.clone(), i);
                self.opening = Some(job.clone());
                // 보기용으로 이미 번역 중인 세트면 그 결과(WatchSet)를 기다린다
                if self.converting.as_ref() == Some(&job) {
                    return Task::none();
                }
                let ai = self.stats.ai();
                return Task::perform(
                    off_thread(move || {
                        claude::lines_from_transcript(&ai, &chunk.text()).map(|mut lines| {
                            youtube::align(&chunk, &mut lines);
                            lines
                        })
                    }),
                    move |r| Msg::SetReady(id.clone(), i, r),
                );
            }
            Msg::SetReady(id, i, res) => {
                self.busy = None;
                self.opening = None;
                match res {
                    Ok(lines) => {
                        if let Some(set) =
                            self.library.video_mut(&id).and_then(|v| v.sets.get_mut(i))
                        {
                            *set = Some(lines.clone());
                        }
                        self.save_library();
                        if self.watching.as_ref() == Some(&id) {
                            self.push_subs();
                        }
                        self.cur_video = Some((id, i));
                        return self.begin(lines);
                    }
                    Err(e) => self.error = Some(e),
                }
            }
            Msg::OpenLesson(i) => {
                if let Some(l) = self.library.lessons.get(i) {
                    let (lines, genre) = (l.lines.clone(), l.genre);
                    self.cur_video = None;
                    let task = self.begin(lines);
                    self.lesson_genre = genre;
                    return task;
                }
            }
            Msg::WriteLesson(i) => {
                if let Some(l) = self.library.lessons.get(i) {
                    let (lines, genre) = (l.lines.clone(), l.genre);
                    self.cur_video = None;
                    self.reset(lines);
                    self.lesson_genre = genre;
                    self.screen = Screen::Writing;
                    return self.show_writing();
                }
            }
            Msg::Section(s) => {
                self.section = s;
                self.confirm_delete = None;
                return to_top();
            }
            Msg::ToggleSettings => {} // 앱 셸(main.rs)이 설정 창을 연다
            Msg::AskDelete(d) => self.confirm_delete = Some(d),
            Msg::CancelDelete => self.confirm_delete = None,
            Msg::DeleteLesson(i) => {
                self.confirm_delete = None;
                if i < self.library.lessons.len() {
                    self.library.lessons.remove(i);
                    self.save_library();
                }
            }
            Msg::DeleteVideo(id) => {
                self.confirm_delete = None;
                self.library.videos.retain(|v| v.id != id);
                self.save_library();
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
                    // 처음이면 서버를 띄우고 브라우저를 연다
                    None if self.watching.is_some() && self.screen == Screen::Setup => {
                        return self.start_watch();
                    }
                    None => self.play_current(),
                }
                return operation::focus(INPUT_ID);
            }
            Msg::Cookies(on) => {
                self.stats.no_yt_cookies = !on;
                self.save_stats();
            }
            Msg::Drag => return window::oldest().and_then(window::drag),
            Msg::Maximize => return window::oldest().and_then(window::toggle_maximize),
            Msg::DismissError => self.error = None,
            // Esc 는 가장 위에 떠 있는 것부터 닫는다: 오류 → 따라치기
            Msg::Escape => {
                if self.error.take().is_some() {
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

    /// API 키가 있으면 Claude 모델 목록을 실시간으로 한 번 받는다(다시 받기는 설정의 새로고침).
    /// Claude 엔진(Claude Code · API)일 때만. Codex 는 자기 목록이 있다.
    pub(crate) fn load_models(&mut self) -> Task<Msg> {
        let claude = self.stats.engine != Engine::Codex;
        if !claude || self.models.is_some() || self.key_source == KeySource::Missing {
            return Task::none();
        }
        self.models = Some(Ok(0)); // 받는 중: 같은 요청을 두 번 보내지 않게
        Task::perform(off_thread(claude::refresh_models), Msg::ModelsLoaded)
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
        self.lesson_genre = None;
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
        self.save_stats();
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
        self.save_library();
    }

    /// 보일 때만: 따라치기 화면이면 현재 문장을 재생하고 입력칸에 포커스.
    /// 숨어 있을 때 AI·유튜브 결과가 도착하면 여기서 멈추고, 영어 공부 모드로 돌아올 때 앱 셸이 다시 부른다.
    /// (iced 의 focus 연산은 다른 위젯의 포커스를 모두 해제하므로, 숨은 채로 부르면 글쓰기 편집기의 포커스를 뺏는다)
    pub(crate) fn resume(&mut self) -> Task<Msg> {
        if self.visible && self.watch_pending {
            self.watch_pending = false;
            return self.start_watch();
        }
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
        let (id, start, end) = (id.clone(), line.start, line.end);
        let rate = if self.slow { 0.75 } else { 1.0 };
        if let Some(p) = self.ensure_player() {
            p.play(&id, start, end, rate);
        }
    }

    /// 플레이어 서버. 없으면 띄우고 브라우저로 연다.
    fn ensure_player(&mut self) -> Option<&Player> {
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
                    return None;
                }
            }
        }
        self.player.as_ref()
    }

    /// 보던 영상을 플레이어에 띄우고 자막을 넘긴 뒤, 아직 번역 안 된 세트를 번역하기 시작.
    fn start_watch(&mut self) -> Task<Msg> {
        let Some(id) = self.watching.clone() else {
            return Task::none();
        };
        if let Some(p) = self.ensure_player() {
            p.watch(&id);
        }
        self.push_subs();
        self.convert_next()
    }

    /// 보기 자막을 플레이어에 넘긴다.
    fn push_subs(&self) {
        if let (Some(p), Some(v)) = (
            &self.player,
            self.watching.as_ref().and_then(|id| self.library.video(id)),
        ) {
            p.set_subs(watch_subs(v));
        }
    }

    /// 번역 안 된 첫 세트를 뒤에서 번역(한 번에 하나). 다 됐으면 아무것도 안 한다.
    fn convert_next(&mut self) -> Task<Msg> {
        if self.converting.is_some() {
            return Task::none();
        }
        let Some(v) = self.watching.as_ref().and_then(|id| self.library.video(id)) else {
            return Task::none();
        };
        // 따라치기가 따로 번역 중인 세트는 건너뛴다
        let opening = |i: usize| {
            self.opening
                .as_ref()
                .is_some_and(|(oid, oi)| *oid == v.id && *oi == i)
        };
        let Some(i) = (0..v.sets.len()).find(|&i| v.sets[i].is_none() && !opening(i)) else {
            return Task::none();
        };
        let (id, chunk, ai) = (v.id.clone(), v.chunks[i].clone(), self.stats.ai());
        self.converting = Some((id.clone(), i));
        Task::perform(
            off_thread(move || {
                claude::lines_from_transcript(&ai, &chunk.text()).map(|mut lines| {
                    youtube::align(&chunk, &mut lines);
                    lines
                })
            }),
            move |r| Msg::WatchSet(id.clone(), i, r),
        )
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
            if let Some((id, i)) = &self.cur_video
                && let Some(v) = self.library.video_mut(id)
            {
                v.progress = v.progress.max(i + 1);
            }
            self.save_library();
        }
        self.save_stats();
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
            container(container(body).max_width(if self.screen == Screen::Setup {
                640
            } else {
                720
            }))
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
        // 처음 화면만 왼쪽에 기능 목록. 따라치기 · 직접 쓰기 중엔 숨겨서 집중
        let body: Element<'_, Msg> = if self.screen == Screen::Setup {
            row![self.sidebar(p), body].into()
        } else {
            body.into()
        };

        container(column![titlebar, body, self.status_bar(p)])
            .style(move |_| container::Style {
                background: Some(p.bg.into()),
                ..Default::default()
            })
            .width(Fill)
            .height(Fill)
            .into()
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
        bar = bar.push(text(self.stats.ai().label()).size(12).color(p.muted));
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
            Screen::Writing if self.total_chars == 0 => {} // 바로 영작: 따라친 기록 없음
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

    /// 처음 화면의 왼쪽 사이드바: 기능 목록 + 아래에 설정.
    fn sidebar(&self, p: Palette) -> Element<'_, Msg> {
        let due = self.library.due(today(), usize::MAX).len();
        let items = Column::with_children(Section::ALL.map(|s| {
            let mut r = row![text(s.label()).size(14)].align_y(Center);
            if s == Section::Review && due > 0 {
                r = r
                    .push(space::horizontal())
                    .push(text(due.to_string()).size(12).color(p.accent));
            }
            button(r)
                .width(Fill)
                .style(side(p, self.section == s))
                .padding([7, 12])
                .on_press(Msg::Section(s))
                .into()
        }))
        .spacing(2);
        container(
            column![
                items,
                space::vertical(),
                button(text("설정  ⌘,").size(13))
                    .width(Fill)
                    .style(side(p, false))
                    .padding([7, 12])
                    .on_press(Msg::ToggleSettings),
            ]
            .height(Fill),
        )
        .width(168)
        .height(Fill)
        .padding(Padding {
            top: 8.0,
            bottom: 12.0,
            left: 12.0,
            right: 8.0,
        })
        .into()
    }

    /// 처음 화면: 사이드바에서 고른 기능 하나만 보여 준다.
    fn setup_view(&self, p: Palette) -> Element<'_, Msg> {
        match self.section {
            Section::Review => self.review_section(p),
            Section::YouTube => self.youtube_section(p),
            Section::Ai => self.ai_section(p),
            Section::Write => self.write_section(p),
            Section::Library => self.library_section(p),
            Section::Records => self.records_section(p),
        }
    }

    fn review_section(&self, p: Palette) -> Element<'_, Msg> {
        let idle = self.busy.is_none();
        let t = today();
        let due = self.library.due(t, usize::MAX).len();
        let deck = self.library.deck.len();
        let status: Element<'_, Msg> = if due > 0 {
            card(
                p,
                row![
                    column![
                        text(format!("오늘 복습할 문장 {due}개"))
                            .size(18)
                            .color(p.strong),
                        text(format!(
                            "한국어만 보고 영어로 써봅니다 · 한 번에 {}문장",
                            due.min(REVIEW_MAX)
                        ))
                        .size(13)
                        .color(p.muted),
                    ]
                    .spacing(4)
                    .width(Fill),
                    button(text("복습 시작"))
                        .style(primary(p))
                        .on_press_maybe(idle.then_some(Msg::StartReview(false)))
                        .padding([10, 24]),
                ]
                .spacing(12)
                .align_y(Center),
            )
        } else if deck > 0 {
            let next = self.library.deck.iter().map(|c| c.due).min().unwrap_or(t);
            row![
                text(format!(
                    "오늘은 복습할 문장이 없습니다 · 카드 {deck}장 · 다음 복습은 {}일 뒤",
                    next.saturating_sub(t)
                ))
                .size(14)
                .color(p.fg)
                .width(Fill),
                button(text(format!("미리 복습 ({}문장)", deck.min(REVIEW_MAX))).size(13))
                    .style(pill(p, false))
                    .on_press_maybe(idle.then_some(Msg::StartReview(true)))
                    .padding([6, 14]),
            ]
            .spacing(12)
            .align_y(Center)
            .into()
        } else {
            text("아직 복습 카드가 없습니다. 유튜브나 AI 문장으로 따라치기를 끝까지 하면 그 문장들이 카드가 되고, 다음 날부터 여기에 나옵니다.")
                .size(14)
                .color(p.fg)
                .into()
        };
        column![
            page_title(p, "복습", "며칠 전에 따라친 문장을 한국어만 보고 다시 써봅니다."),
            status,
            guide(
                p,
                &[
                    "따라치기를 마친 문장이 복습 카드가 됩니다 (다음 날부터).",
                    "첨삭에서 고칠 곳이 나온 문장은 곧 다시 나옵니다. 구두점만 틀린 건 맞은 것으로 칩니다.",
                    "맞힌 문장은 1 → 3 → 7 → 14 → 30일 간격으로 나옵니다. 틀리면 한 칸 내려갑니다.",
                ]
            ),
        ]
        .spacing(28)
        .into()
    }

    /// 영상: 골라 둔 영어 영상을 영어 · 한국어 자막과 함께 본다. 따라치기는 보관함에서.
    fn youtube_section(&self, p: Palette) -> Element<'_, Msg> {
        let idle = self.busy.is_none();
        let mut page = column![
            page_title(
                p,
                "영상으로 공부",
                "골라 둔 영어 영상을 영어 · 한국어 자막과 함께 봅니다. 문장을 누르면 그 자리로 가고, 문장마다 멈춰 따라 말해 볼 수 있습니다."
            ),
        ]
        .spacing(28);

        // 지금 보는 영상
        if let Some(v) = self.watching.as_ref().and_then(|id| self.library.video(id)) {
            let done = v.sets.iter().filter(|s| s.is_some()).count();
            let status = if done == v.sets.len() {
                "자막 번역 완료".to_string()
            } else if self.converting.is_some() {
                format!("한국어 자막 번역 중 · {done}/{} 세트", v.sets.len())
            } else {
                format!(
                    "한국어 자막 {done}/{} 세트 (번역 멈춤 — 다시 보기를 누르면 이어서)",
                    v.sets.len()
                )
            };
            page =
                page.push(card(
                    p,
                    column![
                    text("지금 보는 중").size(12).color(p.muted),
                    text(&v.title).size(18).color(p.strong),
                    text(status).size(13).color(p.muted),
                    row![
                        button(text("플레이어 열기"))
                            .style(primary(p))
                            .on_press(Msg::OpenPlayer)
                            .padding([8, 18]),
                        button(text("다시 보기 (이어서 번역)").size(13))
                            .style(pill(p, false))
                            .on_press(Msg::Watch(v.id.clone()))
                            .padding([8, 14]),
                        button(text("이 영상 따라치기").size(13))
                            .style(pill(p, false))
                            .on_press_maybe(idle.then(|| Msg::OpenSet(v.id.clone(), resume_set(v))))
                            .padding([8, 14]),
                        button(text("그만 보기").size(13))
                            .style(quiet(p))
                            .on_press(Msg::StopWatch)
                            .padding([8, 10]),
                    ]
                    .spacing(8)
                    .align_y(Center)
                    .wrap(),
                    text("플레이어: Space 재생/멈춤 · ←→ 문장 이동 · R 다시 듣기 · K 한국어 숨기기 · 다음에 열면 지난번 본 곳부터")
                        .size(12)
                        .color(p.muted),
                ]
                    .spacing(8),
                ));
        }

        // 골라 둔 영상
        let row_for = |id: &'static str, title: &'static str, sub: String| -> Element<'_, Msg> {
            let saved = self.library.video(id).is_some();
            row![
                column![
                    text(title).size(15).color(p.strong).font(self.en_font()),
                    text(sub).size(12).color(p.muted),
                ]
                .spacing(2)
                .width(Fill),
                button(
                    text(if saved {
                        "보기"
                    } else {
                        "자막 받고 보기"
                    })
                    .size(13)
                )
                .style(pill(p, self.watching.as_deref() == Some(id)))
                .on_press_maybe(idle.then(|| Msg::Watch(id.into())))
                .padding([6, 14]),
            ]
            .spacing(12)
            .align_y(Center)
            .padding([6, 0])
            .into()
        };
        for (group, picks) in youtube::CURATED {
            let rows = Column::with_children(picks.iter().map(|k| {
                row_for(
                    k.id,
                    k.title,
                    format!("{} · {} · {}", k.channel, k.len, k.note),
                )
            }))
            .spacing(2);
            page = page.push(section(p, group, "", rows));
        }

        // 내 영상: 링크로 추가한 것(골라 둔 목록에 없는 것)
        let curated = |id: &str| {
            youtube::CURATED
                .iter()
                .any(|(_, ks)| ks.iter().any(|k| k.id == id))
        };
        let mine =
            Column::with_children(self.library.videos.iter().filter(|v| !curated(&v.id)).map(
                |v| {
                    row![
                        text(&v.title).size(15).color(p.strong).width(Fill),
                        button(text("보기").size(13))
                            .style(pill(p, self.watching.as_ref() == Some(&v.id)))
                            .on_press_maybe(idle.then(|| Msg::Watch(v.id.clone())))
                            .padding([6, 14]),
                    ]
                    .spacing(12)
                    .align_y(Center)
                    .padding([6, 0])
                    .into()
                },
            ))
            .spacing(2);
        page = page.push(section(
            p,
            "링크로 보기",
            "다른 영상은 링크를 넣으세요. 영어 자막(자동 생성 포함)이 있어야 합니다.",
            column![
                row![
                    text_input("https://www.youtube.com/watch?v=…", &self.yt_url)
                        .on_input(Msg::YtUrl)
                        .on_submit_maybe(idle.then_some(Msg::YtLoad))
                        .style(field(p))
                        .padding(10),
                    button(text("보기"))
                        .style(primary(p))
                        .on_press_maybe(
                            (idle && !self.yt_url.trim().is_empty()).then_some(Msg::YtLoad)
                        )
                        .padding([10, 22]),
                ]
                .spacing(8),
                mine,
            ]
            .spacing(8),
        ));
        page = page.push(
            text("받아쓰기(문장마다 멈추고 따라 치기)는 \"보관함\"의 영상에서 \"이어하기\"로 합니다.")
                .size(12)
                .color(p.muted),
        );
        page.into()
    }

    fn ai_section(&self, p: Palette) -> Element<'_, Msg> {
        let idle = self.busy.is_none();
        let levels = row(LEVELS
            .iter()
            .map(|l| toggle(p, l, self.level == *l, Msg::Level(l))))
        .spacing(4);
        let genre = row(Genre::ALL.map(|g| toggle(p, g.label(), self.genre == g, Msg::Genre(g))))
            .spacing(4);
        // 예시 주제: 누르면 주제 칸에
        let examples = row(self.genre.examples().iter().map(|&ex| {
            button(text(ex).size(12).font(self.en_font()))
                .style(pill(p, self.topic == ex))
                .on_press(Msg::Topic(ex.into()))
                .padding([4, 10])
                .into()
        }))
        .spacing(4)
        .wrap()
        .vertical_spacing(4);
        let desc = match self.genre {
            Genre::Prompt => {
                "개발할 때 Claude Code에 보내는 영어 프롬프트를 만듭니다. 따라 친 뒤 한국어만 보고 프롬프트를 직접 써 보면, 첨삭이 문법과 함께 지시가 명확한지도 봐 줍니다."
            }
            _ => "레벨과 주제를 고르면 AI가 8~12줄을 만들어 줍니다.",
        };
        let topic = text_input("직접 쓰거나 아래 예시를 누르세요", &self.topic)
            .on_input(Msg::Topic)
            .on_submit_maybe(idle.then_some(Msg::Generate))
            .style(field(p))
            .padding(10);
        column![
            page_title(p, "AI 문장으로 연습", desc),
            column![
                labeled(p, "레벨", levels.into()),
                labeled(p, "장르", genre.into()),
                labeled(p, "주제", topic.into()),
                labeled(p, "예시", examples.into()),
                button(text("문장 만들기"))
                    .style(primary(p))
                    .on_press_maybe(idle.then_some(Msg::Generate))
                    .padding([10, 24]),
            ]
            .spacing(12),
        ]
        .spacing(28)
        .into()
    }

    /// 직접 쓰기: 첨삭 · 다시 쓰기 · 오류 유형이 여기서 나온다.
    fn write_section(&self, p: Palette) -> Element<'_, Msg> {
        let idle = self.busy.is_none();
        let body: Element<'_, Msg> = if self.library.lessons.is_empty() {
            text("보관함에 AI 문장이 없습니다. 'AI 문장'에서 먼저 만들어 주세요. 따라치기를 끝내도 이 화면으로 이어집니다.")
                .size(14)
                .color(p.fg)
                .into()
        } else {
            Column::with_children(self.library.lessons.iter().enumerate().map(|(i, l)| {
                row![
                    column![
                        text(&l.title).size(15).color(p.strong),
                        text(format!("{}줄", l.lines.len())).size(12).color(p.muted),
                    ]
                    .spacing(2)
                    .width(Fill),
                    button(text("바로 영작").size(13))
                        .style(pill(p, false))
                        .on_press_maybe(idle.then_some(Msg::WriteLesson(i)))
                        .padding([6, 14]),
                ]
                .align_y(Center)
                .padding([8, 0])
                .into()
            }))
            .spacing(2)
            .into()
        };
        column![
            page_title(
                p,
                "직접 쓰기",
                "한국어 뜻만 보고 영어로 쓰고 AI 첨삭을 받습니다. 따라치기 없이 바로 시작할 수 있습니다."
            ),
            guide(
                p,
                &[
                    "첨삭 받기 → 교정문 · 고친 곳 · 총평 · 모범 답안이 나옵니다.",
                    "오류 유형: 고친 곳마다 관사 · 시제 같은 유형이 붙고, 이번에 두 번 이상 나온 유형을 '이번에 집중할 것'으로 먼저 보여 줍니다.",
                    "다시 쓰기: 첨삭 뒤 '교정문 가리고 다시 쓰기'를 누르면 한 번 더 쓰고, 고칠 곳이 몇 개 줄었는지 비교합니다.",
                ]
            ),
            section(p, "보관함의 AI 문장", "", body),
        ]
        .spacing(28)
        .into()
    }

    fn library_section(&self, p: Palette) -> Element<'_, Msg> {
        column![
            page_title(
                p,
                "보관함",
                "만든 문장과 영상은 자동 저장됩니다. 다시 열면 AI를 부르지 않고 바로 시작합니다."
            ),
            self.library_rows(p, true, true),
        ]
        .spacing(28)
        .into()
    }

    /// 보관함 목록 (영상 · AI 문장). 비었으면 "아직 없음".
    fn library_rows(&self, p: Palette, videos: bool, lessons: bool) -> Element<'_, Msg> {
        let idle = self.busy.is_none();
        let confirming = |d: &Del| self.confirm_delete.as_ref() == Some(d);
        let mut items = Column::new().spacing(2);
        for v in self.library.videos.iter().filter(|_| videos) {
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
        for (i, l) in self.library.lessons.iter().enumerate().filter(|_| lessons) {
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
        let empty = (!videos || self.library.videos.is_empty())
            && (!lessons || self.library.lessons.is_empty());
        if empty {
            text("아직 없음").size(14).color(p.muted).into()
        } else {
            items.into()
        }
    }

    /// 기록: 숫자 · 따라치기 오타 · 첨삭 오류 유형을 한 곳에
    fn records_section(&self, p: Palette) -> Element<'_, Msg> {
        let (en, ko) = (self.en_font(), self.ko_font());
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
            "누적 {}줄 · 최고 {:.0} WPM · 복습 카드 {}장",
            self.stats.lines_typed,
            self.stats.best_wpm,
            self.library.deck.len()
        );
        column![
            page_title(
                p,
                "기록",
                "실수 유형은 첨삭을 받을 때 쌓입니다(처음 쓴 글만 셉니다)."
            ),
            column![
                stat("따라치기", totals, UI_FONT),
                stat("틀린 단어", missed, en),
                stat("실수 유형", kinds, ko),
            ]
            .spacing(10),
        ]
        .spacing(28)
        .into()
    }
    /// 설정 창의 "영어 공부": AI 엔진 · 유튜브 · 글꼴. 바꾸면 바로 저장된다.
    pub(crate) fn settings_view(&self, p: Palette) -> Element<'_, Msg> {
        let (en, ko) = (self.en_font(), self.ko_font());
        let idle = self.busy.is_none();
        let small = |t: &'static str| text(t).size(12).color(p.muted);
        let heading = |t: &'static str| text(t).size(14).color(p.strong);

        let ai = self.stats.ai();
        let engines = row(Engine::ALL.map(|e| {
            button(text(e.name()).size(13))
                .style(pill(p, ai.engine == e))
                .on_press_maybe(idle.then_some(Msg::Engine(e)))
                .padding([5, 14])
                .into()
        }))
        .spacing(4);
        let models = row(ai.engine.models().into_iter().map(|(id, label)| {
            let on = id == ai.model;
            button(text(label).size(13))
                .style(pill(p, on))
                .on_press_maybe(idle.then_some(Msg::Model(id)))
                .padding([5, 12])
                .into()
        }))
        .spacing(4)
        .wrap()
        .vertical_spacing(4);
        let refresh = ai.engine != Engine::Codex && self.key_source != KeySource::Missing;
        let mut model_head = row![
            text("모델").size(12).color(p.muted),
            text(match (&self.models, ai.engine) {
                (_, Engine::Codex) => "Codex 가 받아 둔 목록".to_string(),
                (Some(Ok(0)), _) => "API에서 받는 중…".to_string(),
                (Some(Ok(n)), _) => format!("API 실시간 목록 · {n}개"),
                (Some(Err(e)), _) => format!("기본 목록 (실시간 목록 실패: {e})"),
                (None, _) => "기본 목록 · API 키를 저장하면 실시간 목록을 받습니다".to_string(),
            })
            .size(12)
            .color(p.muted)
            .width(Fill),
        ]
        .spacing(8)
        .align_y(Center);
        if refresh {
            model_head = model_head.push(
                button(text("새로고침").size(12))
                    .style(quiet(p))
                    .on_press(Msg::RefreshModels)
                    .padding([2, 8]),
            );
        }
        let mut connect = column![
            heading("AI 엔진"),
            small("문장 만들기 · 자막 정리 · 첨삭에 씁니다."),
            engines,
            small(ai.engine.how()),
            model_head,
            models,
        ]
        .spacing(8);
        if ai.engine == Engine::Api {
            let status = match self.key_source {
                KeySource::Env => "API 키: ANTHROPIC_API_KEY 환경변수를 씁니다",
                KeySource::Keychain => "API 키: 키체인에 저장됨",
                KeySource::Missing => {
                    "API 키: 없음 — console.anthropic.com 에서 만든 키를 붙여 넣으세요"
                }
            };
            let mut keys = row![
                text_input("sk-ant-…", &self.api_key)
                    .secure(true)
                    .on_input(Msg::ApiKey)
                    .on_submit(Msg::SaveKey)
                    .style(field(p))
                    .padding(8)
                    .size(13),
                button(text("저장").size(13))
                    .style(primary(p))
                    .on_press_maybe((!self.api_key.trim().is_empty()).then_some(Msg::SaveKey))
                    .padding([6, 14]),
            ]
            .spacing(6)
            .align_y(Center);
            if self.key_source == KeySource::Keychain {
                keys = keys.push(
                    button(text("지우기").size(13))
                        .style(quiet(p))
                        .on_press(Msg::DeleteKey)
                        .padding([6, 10]),
                );
            }
            connect = connect.push(text(status).size(12).color(p.fg)).push(keys);
        }
        let result: Element<'_, Msg> = match &self.checked {
            Some(Ok(label)) => text(format!("✓ 연결됨 · {label}"))
                .size(12)
                .color(p.ok)
                .into(),
            Some(Err(e)) => text(format!("⚠ {e}")).size(12).color(p.bad).into(),
            None => space::horizontal().width(0).into(),
        };
        connect = connect.push(
            row![
                button(text("연결 확인").size(13))
                    .style(pill(p, false))
                    .on_press_maybe(idle.then_some(Msg::CheckAi))
                    .padding([5, 14]),
                result
            ]
            .spacing(10)
            .align_y(Center),
        );
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
            connect,
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
        } else if self.total_chars == 0 {
            // 보관함에서 바로 영작 (따라친 글자가 없음)
            column![
                text("직접 쓰기").size(26).color(p.strong),
                text(format!(
                    "{}문장 · 한국어만 보고 영어로 써보세요",
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
        let prompt = self.lesson_genre == Some(Genre::Prompt);
        let editor = text_editor(&self.editor)
            .id(WRITE_ID)
            .placeholder(match (rewriting, prompt) {
                (true, _) => "교정문을 떠올리며 다시 써보세요…",
                (false, true) => {
                    "Claude Code에 보낼 프롬프트를 영어로 써보세요… (예: Please add a test for …)"
                }
                (false, false) => "영어로 써보세요…",
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
        } else if prompt {
            (
                "이제 프롬프트를 직접 써보세요",
                "한국어 뜻을 보고 Claude Code에 보낼 영어 프롬프트를 써보세요. 무엇을 · 어디서 · 어떻게 확인할지까지 적으면 좋습니다. 첨삭이 문법과 함께 지시가 명확한지도 봐 줍니다.",
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

/// 보기 자막: 번역된 세트는 문장(영어+한국어), 아직이면 원문 단어를 12개씩 끊은 영어만.
// ponytail: 12단어 고정 끊기. 번역 전 잠깐 보이는 자막이라 문장 경계까지 맞추지 않는다.
fn watch_subs(v: &Video) -> serde_json::Value {
    let mut out = vec![];
    for (chunk, set) in v.chunks.iter().zip(&v.sets) {
        match set {
            Some(lines) => out.extend(lines.iter().map(|l| {
                serde_json::json!({"start": l.start, "end": l.end, "en": format!("{}{}", l.who(), l.en), "ko": l.ko})
            })),
            None => {
                let groups: Vec<_> = chunk.words.chunks(12).collect();
                for (k, g) in groups.iter().enumerate() {
                    let end = groups.get(k + 1).map_or(chunk.end, |n| n[0].0);
                    let en: Vec<_> = g.iter().map(|w| w.1.as_str()).collect();
                    out.push(serde_json::json!({"start": g[0].0, "end": end, "en": en.join(" "), "ko": ""}));
                }
            }
        }
    }
    serde_json::Value::Array(out)
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

/// 사이드바 항목: 고른 것만 옅은 배경
fn side(p: Palette, on: bool) -> ButtonStyle {
    Box::new(move |_, st| button::Style {
        background: (on || st == button::Status::Hovered).then(|| {
            Color {
                a: if on { 0.12 } else { 0.06 },
                ..p.fg
            }
            .into()
        }),
        text_color: if on { p.strong } else { p.fg },
        border: Border::default().rounded(8),
        ..Default::default()
    })
}

/// 기능 화면 맨 위 제목 + 한 줄 설명
fn page_title<'a>(p: Palette, title: &'a str, desc: &'a str) -> Element<'a, Msg> {
    column![
        text(title).size(28).color(p.strong),
        text(desc).size(14).color(p.muted)
    ]
    .spacing(4)
    .into()
}

/// 이 기능이 어떻게 동작하는지 짧은 안내 목록
fn guide<'a>(p: Palette, lines: &[&'a str]) -> Element<'a, Msg> {
    Column::with_children(
        lines
            .iter()
            .map(|&l| text(format!("· {l}")).size(13).color(p.muted).into()),
    )
    .spacing(6)
    .into()
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
    p.error = Some("x".into());
    let _ = p.update(Msg::Escape);
    assert!(p.error.is_none() && p.screen == Screen::Typing);
    let _ = p.update(Msg::Escape);
    assert!(p.screen == Screen::Setup);

    // 삭제: 한 번 누르면 묻기만 하고, 취소하면 그대로
    p.library.lessons.push(Lesson {
        title: "t".into(),
        lines: vec![],
        genre: None,
    });
    let n = p.library.lessons.len();
    let _ = p.update(Msg::AskDelete(Del::Lesson(n - 1)));
    assert_eq!(p.library.lessons.len(), n);
    let _ = p.update(Msg::CancelDelete);
    assert!(p.confirm_delete.is_none() && p.library.lessons.len() == n);
}

#[test]
fn library_lesson_goes_straight_to_writing() {
    let mut p = Practice::new();
    p.library.lessons.insert(
        0,
        Lesson {
            title: "t".into(),
            lines: vec![Line {
                speaker: String::new(),
                en: "Hi.".into(),
                ko: "안녕.".into(),
                start: 0.0,
                end: 0.0,
            }],
            genre: Some(Genre::Prompt),
        },
    );
    let _ = p.update(Msg::Section(Section::Write));
    let _ = p.update(Msg::WriteLesson(0));
    assert!(p.screen == Screen::Writing && p.review_cards.is_empty() && p.total_chars == 0);
    assert_eq!(
        p.lesson_genre,
        Some(Genre::Prompt),
        "프롬프트 안내를 보여 준다"
    );
    let _ = p.update(Msg::Home);
    assert!(
        p.screen == Screen::Setup && p.section == Section::Write,
        "돌아오면 고른 기능 그대로"
    );
}

#[test]
fn watching_shows_raw_english_until_translated_and_waits_while_hidden() {
    let words: Vec<_> = (0..20).map(|i| (i as f64, format!("w{i}"))).collect();
    let chunk = youtube::Chunk { words, end: 20.0 };
    let line = Line {
        speaker: String::new(),
        en: "Hello there.".into(),
        ko: "안녕하세요.".into(),
        start: 20.0,
        end: 22.0,
    };
    let v = Video {
        id: "abcdefghijk".into(),
        title: "t".into(),
        chunks: vec![chunk.clone(), chunk],
        sets: vec![None, Some(vec![line])],
        progress: 0,
    };
    let subs = watch_subs(&v);
    let subs = subs.as_array().unwrap();
    // 번역 전 세트: 20단어 → 12 + 8, 한국어 없음. 번역된 세트: 문장 그대로
    assert_eq!(subs.len(), 3);
    assert_eq!(
        (subs[0]["start"].as_f64(), subs[0]["end"].as_f64()),
        (Some(0.0), Some(12.0))
    );
    assert_eq!(
        subs[1]["end"].as_f64(),
        Some(20.0),
        "마지막 묶음은 세트 끝까지"
    );
    assert_eq!(
        (subs[2]["en"].as_str(), subs[2]["ko"].as_str()),
        (Some("Hello there."), Some("안녕하세요."))
    );

    // 숨어 있을 때 보기: 브라우저를 열지 않고, 보일 때 연다
    let mut p = Practice::new();
    p.library.videos.insert(0, v);
    let _ = p.update(Msg::Watch("abcdefghijk".into()));
    assert!(p.player.is_none() && p.watch_pending && p.watching.is_some());
    let _ = p.update(Msg::StopWatch);
    assert!(p.watching.is_none());
}

#[test]
fn background_translation_follows_the_watched_video_and_is_never_doubled() {
    let vid = |id: &str, n: usize| Video {
        id: id.into(),
        title: id.into(),
        chunks: vec![
            youtube::Chunk {
                words: vec![(0.0, "hi".into())],
                end: 1.0
            };
            n
        ],
        sets: vec![None; n],
        progress: 0,
    };
    let (a, b) = ("AAAAAAAAAAA".to_string(), "BBBBBBBBBBB".to_string());
    let line = || Line {
        speaker: String::new(),
        en: "Hi.".into(),
        ko: "안녕.".into(),
        start: 0.0,
        end: 1.0,
    };

    // A 번역 중에 B 로 바꿈 → A 가 끝나면 B 를 이어서 번역한다
    let mut p = Practice::new();
    p.library.videos = vec![vid(&a, 1), vid(&b, 1)];
    p.converting = Some((a.clone(), 0));
    p.watching = Some(b.clone());
    let _ = p.update(Msg::WatchSet(a.clone(), 0, Ok(vec![line()])));
    assert_eq!(p.converting, Some((b.clone(), 0)), "B 번역이 시작돼야 한다");
    // 옛 영상 번역이 실패해도 지금 영상은 시작
    p.converting = Some((a.clone(), 0));
    p.library.videos[0].sets[0] = None;
    let _ = p.update(Msg::WatchSet(a.clone(), 0, Err("x".into())));
    assert_eq!(p.converting, Some((b.clone(), 0)));

    // 뒤에서 번역 중인 세트를 따라치기로 열면: 다시 부르지 않고 기다렸다가 그 결과로 연다
    let mut p = Practice::new();
    p.library.videos = vec![vid(&a, 2)];
    p.watching = Some(a.clone());
    p.converting = Some((a.clone(), 0));
    let _ = p.update(Msg::OpenSet(a.clone(), 0));
    assert!(p.busy.is_some() && p.opening == Some((a.clone(), 0)));
    let _ = p.update(Msg::WatchSet(a.clone(), 0, Ok(vec![line()])));
    assert!(p.screen == Screen::Typing && p.busy.is_none() && p.opening.is_none());
    assert_eq!(
        p.converting,
        Some((a.clone(), 1)),
        "보기 번역은 다음 세트로"
    );

    // 따라치기가 따로 번역 중인 세트는 뒤 번역이 건너뛴다
    let mut p = Practice::new();
    p.library.videos = vec![vid(&a, 2)];
    p.watching = Some(a.clone());
    p.opening = Some((a.clone(), 0));
    let _ = p.convert_next();
    assert_eq!(p.converting, Some((a, 1)));
}

#[test]
fn early_review_starts_before_the_due_date() {
    let mut p = Practice::new();
    let line = Line {
        speaker: String::new(),
        en: "Hi.".into(),
        ko: "안녕.".into(),
        start: 0.0,
        end: 0.0,
    };
    p.library.deck.clear();
    p.library.add_to_deck(&[line], today()); // 내일부터 복습
    let _ = p.update(Msg::StartReview(false));
    assert!(p.screen == Screen::Setup, "오늘은 복습할 게 없다");
    let _ = p.update(Msg::StartReview(true));
    assert!(
        p.screen == Screen::Writing && p.review_cards == [0],
        "미리 복습은 바로 시작"
    );
}
