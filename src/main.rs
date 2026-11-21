// iamWriter — 집중 글쓰기 + 영어 공부 (iced)
// 메인 창 하나에 두 모드(⌘1 집중 글쓰기 · ⌘2 영어 공부) + 따로 뜨는 설정 창(⌘,). 설계: docs/TYPINGENG_PORT.md
mod practice;
mod theme;
mod writer;

use iced::{
    Border, Center, Color, Element, Event, Fill, Length, Padding, Subscription, Task, event,
    keyboard, system,
    widget::{Column, button, column, container, row, scrollable, stack, text},
    window,
};
use practice::Practice;
use theme::{Palette, TITLEBAR, UI_FONT};
use writer::Writer;

fn main() -> iced::Result {
    // daemon: 창을 여러 개 띄울 수 있다. 창을 직접 열고(boot), 메인 창을 닫을 때 직접 끝낸다(iced::exit).
    iced::daemon(App::boot, App::update, App::view)
        .title(App::title)
        .theme(App::theme)
        .default_font(UI_FONT)
        .subscription(App::subscription)
        .run()
}

fn main_window() -> window::Settings {
    window::Settings {
        size: iced::Size::new(1000.0, 760.0),
        exit_on_close_request: false, // 닫기 가드를 직접 처리
        #[cfg(target_os = "macos")]
        platform_specific: window::settings::PlatformSpecific {
            title_hidden: true,
            titlebar_transparent: true,
            fullsize_content_view: true, // 본문 배경이 타이틀바 뒤까지
        },
        ..Default::default()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    /// 집중 글쓰기: 방해 없는 마크다운 편집기
    Write,
    /// 영어 공부: 따라치기 + 영작 첨삭 (typingEng)
    Practice,
}

/// 설정 창 왼쪽의 분류
#[derive(Clone, Copy, Debug, PartialEq)]
enum Pane {
    Look,
    Write,
    Practice,
    Keys,
}

impl Pane {
    const ALL: [Pane; 4] = [Pane::Look, Pane::Write, Pane::Practice, Pane::Keys];
    fn label(self) -> &'static str {
        match self {
            Pane::Look => "화면",
            Pane::Write => "집중 글쓰기",
            Pane::Practice => "영어 공부",
            Pane::Keys => "단축키",
        }
    }
}

/// 앱 셸: 두 모드의 상태를 들고 메시지를 감싸서 전달만 한다. 창은 메인 + (열려 있으면) 설정.
struct App {
    mode: Mode,
    writer: Writer,
    practice: Practice,
    settings: Option<window::Id>,
    pane: Pane,
    /// 설정 창에 포커스가 있으면 모드의 키보드 구독을 끈다(Tab = 건너뛰기, Esc = 그만하기가 설정 창에서 동작하지 않게)
    settings_focused: bool,
}

#[derive(Debug, Clone)]
enum Message {
    Writer(writer::Message),
    Practice(practice::Msg),
    Switch(Mode),
    OpenSettings,
    Pane(Pane),
    CloseRequested(window::Id),
    Focused(window::Id),
    /// Esc · ⌘W 를 누른 창
    CloseKey(window::Id),
}

impl App {
    fn boot() -> (Self, Task<Message>) {
        let (writer, task) = Writer::boot();
        // 메인 창은 처음 연 창이라 window::oldest() 로 찾는다(끌기 · 확대)
        let (_, open) = window::open(main_window());
        let mut app = Self::new(writer);
        // API 키가 있으면 Claude 모델 목록을 실시간으로 (백그라운드)
        let models = app.practice.load_models().map(Message::Practice);
        (
            app,
            Task::batch([open.discard(), task.map(Message::Writer), models]),
        )
    }

    fn new(writer: Writer) -> Self {
        Self {
            mode: Mode::Write,
            writer,
            practice: Practice::new(),
            settings: None,
            pane: Pane::Write,
            settings_focused: false,
        }
    }

    fn title(&self, id: window::Id) -> String {
        if Some(id) == self.settings {
            return "설정".into();
        }
        match self.mode {
            Mode::Write => self.writer.title(),
            Mode::Practice => self.practice.title(),
        }
    }

    /// 테마는 하나: 집중 글쓰기 설정의 테마를 두 모드와 설정 창이 같이 쓴다.
    fn theme(&self, _id: window::Id) -> iced::Theme {
        self.writer.theme()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            // 두 모드의 "설정" 버튼은 설정 창을 연다
            Message::Writer(writer::Message::ToggleSettings)
            | Message::Practice(practice::Msg::ToggleSettings)
            | Message::OpenSettings => self.open_settings(),
            Message::Writer(m) => self.writer.update(m).map(Message::Writer),
            // 숨어 있는 동안 도착한 AI·유튜브 결과도 여기로 온다. 포커스·재생은 Practice 가 visible 일 때만 한다.
            Message::Practice(m) => self.practice.update(m).map(Message::Practice),
            Message::Switch(mode) => self.switch(mode),
            Message::Pane(p) => {
                self.pane = p;
                Task::none()
            }
            Message::Focused(id) => {
                self.settings_focused = Some(id) == self.settings;
                Task::none()
            }
            Message::CloseKey(id) if Some(id) == self.settings => self.close_settings(),
            Message::CloseKey(_) => Task::none(),
            Message::CloseRequested(id) if Some(id) == self.settings => self.close_settings(),
            // 메인 창을 닫으면 앱을 끝낸다. 영어 공부 모드에서 닫아도 저장 안 된 글을 확인한다.
            Message::CloseRequested(_) => {
                if self.writer.may_close() {
                    iced::exit()
                } else {
                    Task::none()
                }
            }
        }
    }

    /// 열려 있으면 앞으로 가져오고, 없으면 지금 모드의 분류로 연다.
    fn open_settings(&mut self) -> Task<Message> {
        if let Some(id) = self.settings {
            return window::gain_focus(id);
        }
        self.pane = match self.mode {
            Mode::Write => Pane::Write,
            Mode::Practice => Pane::Practice,
        };
        let (id, open) = window::open(window::Settings {
            size: iced::Size::new(760.0, 600.0),
            min_size: Some(iced::Size::new(560.0, 420.0)),
            exit_on_close_request: false,
            ..Default::default()
        });
        self.settings = Some(id);
        open.discard()
    }

    fn close_settings(&mut self) -> Task<Message> {
        self.settings_focused = false;
        self.settings.take().map_or(Task::none(), window::close)
    }

    /// 모드를 바꾸면 위젯 트리가 새로 만들어져 포커스가 사라지므로 직접 되돌린다.
    fn switch(&mut self, mode: Mode) -> Task<Message> {
        if mode == self.mode {
            return Task::none();
        }
        self.mode = mode;
        self.practice.visible = mode == Mode::Practice;
        match mode {
            Mode::Write => {
                self.writer.show_chrome();
                iced::widget::operation::focus("editor")
            }
            // 숨어 있는 동안 미뤄 둔 재생·포커스를 이어서
            Mode::Practice => self.practice.resume().map(Message::Practice),
        }
    }

    fn view(&self, id: window::Id) -> Element<'_, Message> {
        let content = if Some(id) == self.settings {
            self.settings_view()
        } else {
            let body = match self.mode {
                Mode::Write => self.writer.view().map(Message::Writer),
                Mode::Practice => self
                    .practice
                    .view(self.writer.palette())
                    .map(Message::Practice),
            };
            // 모드 전환 버튼은 타이틀바 오른쪽에 겹쳐 그린다. 버튼 밖의 타이틀바는 아래 층이 그대로 받아 창 끌기가 된다.
            stack![body, self.mode_switch()].into()
        };
        Element::new(command_keys::StripCommandText { inner: content })
    }

    /// 설정 창: 왼쪽 분류, 오른쪽 내용. 바꾸면 바로 저장되고 메인 창에 바로 보인다.
    fn settings_view(&self) -> Element<'_, Message> {
        let p = self.writer.palette();
        let nav = Column::with_children(Pane::ALL.map(|pane| {
            let on = self.pane == pane;
            button(text(pane.label()).size(14))
                .width(Fill)
                .padding([7, 12])
                .on_press(Message::Pane(pane))
                .style(move |_, st| button::Style {
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
                .into()
        }))
        .spacing(2);
        let (title, desc, body): (&str, &str, Element<'_, Message>) = match self.pane {
            Pane::Look => (
                "화면",
                "테마는 두 모드와 이 창이 같이 씁니다. 자동은 macOS 다크 모드를 따라갑니다. ⇧⌘D 로 순환.",
                self.writer.settings_theme().map(Message::Writer),
            ),
            Pane::Write => (
                "집중 글쓰기",
                "본문 글꼴 · 크기 · 줄간격 · 본문 폭 · 포커스 모드",
                self.writer.settings_editor().map(Message::Writer),
            ),
            Pane::Practice => (
                "영어 공부",
                "AI 엔진 · 유튜브 · 따라치기와 첨삭에 쓰는 글꼴",
                self.practice.settings_view(p).map(Message::Practice),
            ),
            Pane::Keys => ("단축키", "", shortcuts(p)),
        };
        let content = scrollable(
            column![
                column![
                    text(title).size(22).color(p.strong),
                    text(desc).size(13).color(p.muted)
                ]
                .spacing(4),
                body
            ]
            .spacing(24)
            .padding(Padding {
                top: 24.0,
                bottom: 32.0,
                left: 28.0,
                right: 28.0,
            }),
        )
        .width(Fill)
        .height(Fill);
        container(row![
            container(nav)
                .width(180)
                .height(Fill)
                .padding(12)
                .style(move |_| container::Style {
                    background: Some(p.panel.into()),
                    ..Default::default()
                }),
            content
        ])
        .style(move |_| container::Style {
            background: Some(p.bg.into()),
            ..Default::default()
        })
        .width(Fill)
        .height(Fill)
        .into()
    }

    fn mode_switch(&self) -> Element<'_, Message> {
        let p = self.writer.palette();
        // 집중 글쓰기에서 타이핑하면 상태줄과 함께 흐려진다
        let alpha = if self.mode == Mode::Write {
            self.writer.chrome_alpha()
        } else {
            1.0
        };
        let tab = |label: &'static str, mode: Mode| {
            let on = self.mode == mode;
            button(text(label).size(12))
                .on_press(Message::Switch(mode))
                .padding([3, 10])
                .style(move |_, st| button::Style {
                    background: (on || st == button::Status::Hovered).then(|| {
                        Color {
                            a: if on { 0.14 } else { 0.07 } * alpha,
                            ..p.fg
                        }
                        .into()
                    }),
                    text_color: Color {
                        a: alpha,
                        ..if on { p.strong } else { p.muted }
                    },
                    border: Border::default().rounded(10),
                    ..Default::default()
                })
        };
        container(
            row![
                tab("집중 글쓰기", Mode::Write),
                tab("영어 공부", Mode::Practice)
            ]
            .spacing(2),
        )
        .align_right(Length::Fill)
        .center_y(TITLEBAR)
        .padding([0, 12])
        .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        // 키보드 구독은 보이는 모드 하나만: Tab 이 글쓰기에선 포커스 이동, 영어 공부에선 건너뛰기라 둘 다 켜면 함께 동작한다.
        // 설정 창에 포커스가 있으면 둘 다 끈다.
        let mode = match (self.settings_focused, self.mode) {
            (true, _) => Subscription::none(),
            (_, Mode::Write) => self.writer.subscription().map(Message::Writer),
            (_, Mode::Practice) => self.practice.subscription().map(Message::Practice),
        };
        Subscription::batch([
            mode,
            event::listen_with(app_keys),
            // exit_on_close_request(false) 라서 이게 없으면 창이 닫히지 않는다.
            // map 의 클로저는 아무것도 캡처하면 안 된다(iced 가 컴파일 단계에서 검사).
            window::close_requests().map(Message::CloseRequested),
            system::theme_changes().map(|m| Message::Writer(writer::Message::SystemMode(m))),
        ])
    }
}

/// 설정 창의 단축키 안내
const SHORTCUTS: &[(&str, &[(&str, &str)])] = &[
    (
        "공통",
        &[
            ("⌘1 / ⌘2", "집중 글쓰기 / 영어 공부"),
            ("⌘,", "설정 창"),
            ("Esc · ⌘W", "설정 창 닫기 (설정 창에서)"),
        ],
    ),
    (
        "집중 글쓰기",
        &[
            ("⌘N · ⌘O", "새 글 · 열기"),
            ("⌘S / ⇧⌘S", "저장 / 다른 이름으로 저장"),
            ("⌘Z / ⇧⌘Z", "실행 취소 / 다시 실행"),
            ("⌘B / ⌘I", "굵게 / 기울임"),
            ("⌘⌫ / ⌥⌫", "줄 앞 / 이전 단어까지 삭제"),
            ("⌘F", "문서 찾기 (Enter 다음 · ⇧Enter 이전)"),
            ("⇧⌘F", "포커스 모드"),
            ("⇧⌘D", "테마 순환"),
        ],
    ),
    (
        "영어 공부",
        &[
            (
                "Enter",
                "줄 끝까지 친 뒤 다음 줄 (유튜브: 빈 칸에서 다시 듣기)",
            ),
            ("Tab", "지금 줄 건너뛰기 (기록 제외)"),
            ("Esc", "오류 닫기 → 따라치기 그만하기"),
        ],
    ),
];

fn shortcuts<'a>(p: Palette) -> Element<'a, Message> {
    Column::with_children(SHORTCUTS.iter().map(|&(group, keys)| {
        column![
            text(group).size(14).color(p.strong),
            Column::with_children(keys.iter().map(|&(k, what)| {
                row![
                    text(k).size(13).width(120).color(p.fg),
                    text(what).size(13).color(p.muted)
                ]
                .align_y(Center)
                .into()
            }))
            .spacing(6)
        ]
        .spacing(8)
        .into()
    }))
    .spacing(24)
    .into()
}

/// 앱 단축키(⌘1 · ⌘2 · ⌘,)와 창 포커스 · Esc/⌘W. 한글 자판에서도 물리 키 기준으로 읽는다.
fn app_keys(event: Event, _status: event::Status, window: window::Id) -> Option<Message> {
    match event {
        Event::Window(window::Event::Focused) => Some(Message::Focused(window)),
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Escape),
            ..
        }) => Some(Message::CloseKey(window)),
        Event::Keyboard(keyboard::Event::KeyPressed {
            key,
            modifiers,
            physical_key,
            ..
        }) if modifiers.command() => match key.to_latin(physical_key) {
            Some('1') => Some(Message::Switch(Mode::Write)),
            Some('2') => Some(Message::Switch(Mode::Practice)),
            Some(',') => Some(Message::OpenSettings),
            Some('w') => Some(Message::CloseKey(window)),
            _ => None,
        },
        _ => None,
    }
}

// ───────────────────────── ⌘ 조합의 글자 입력 막기 ─────────────────────────

mod command_keys {
    use super::Message;
    use iced::advanced::{
        Clipboard, Layout, Shell, layout, overlay, renderer,
        widget::{Operation, Tree, Widget, tree},
    };
    use iced::{Element, Event, Length, Rectangle, Size, Vector, keyboard, mouse};

    type R = iced::Renderer;

    /// macOS 는 ⌘ 조합 키에도 글자(text)를 실어 보내고, iced 의 text_input 은 ⌘C·X·V·A 외에는 그 글자를 그대로 입력한다
    /// (⌘1 → 따라치기 칸에 '1', ⌘F → 검색 칸에 'f'). 그래서 ⌘ 가 눌린 키 입력은 글자만 지우고 넘긴다.
    /// 키 자체는 그대로라서 위젯 단축키(⌘C·⌘V…)와 구독으로 처리하는 앱 단축키는 영향이 없다.
    pub(crate) struct StripCommandText<'a> {
        pub(crate) inner: Element<'a, Message>,
    }

    /// 글자를 지운 이벤트. 바꿀 게 없으면 None.
    pub(crate) fn strip(event: &Event) -> Option<Event> {
        let Event::Keyboard(keyboard::Event::KeyPressed {
            modifiers,
            text: Some(_),
            ..
        }) = event
        else {
            return None;
        };
        if !modifiers.command() {
            return None;
        }
        let mut stripped = event.clone();
        if let Event::Keyboard(keyboard::Event::KeyPressed { text, .. }) = &mut stripped {
            *text = None;
        }
        Some(stripped)
    }

    // 상태가 없는 투명한 래퍼: 위젯 트리도 감싼 위젯의 것을 그대로 쓴다.
    impl Widget<Message, iced::Theme, R> for StripCommandText<'_> {
        fn size(&self) -> Size<Length> {
            self.inner.as_widget().size()
        }
        fn size_hint(&self) -> Size<Length> {
            self.inner.as_widget().size_hint()
        }
        fn tag(&self) -> tree::Tag {
            self.inner.as_widget().tag()
        }
        fn state(&self) -> tree::State {
            self.inner.as_widget().state()
        }
        fn children(&self) -> Vec<Tree> {
            self.inner.as_widget().children()
        }
        fn diff(&self, tree: &mut Tree) {
            self.inner.as_widget().diff(tree)
        }

        fn layout(
            &mut self,
            tree: &mut Tree,
            renderer: &R,
            limits: &layout::Limits,
        ) -> layout::Node {
            self.inner.as_widget_mut().layout(tree, renderer, limits)
        }

        fn operate(
            &mut self,
            tree: &mut Tree,
            layout: Layout<'_>,
            renderer: &R,
            op: &mut dyn Operation,
        ) {
            self.inner
                .as_widget_mut()
                .operate(tree, layout, renderer, op);
        }

        fn update(
            &mut self,
            tree: &mut Tree,
            event: &Event,
            layout: Layout<'_>,
            cursor: mouse::Cursor,
            renderer: &R,
            clipboard: &mut dyn Clipboard,
            shell: &mut Shell<'_, Message>,
            viewport: &Rectangle,
        ) {
            let stripped = strip(event);
            let event = stripped.as_ref().unwrap_or(event);
            self.inner.as_widget_mut().update(
                tree, event, layout, cursor, renderer, clipboard, shell, viewport,
            );
        }

        fn draw(
            &self,
            tree: &Tree,
            renderer: &mut R,
            theme: &iced::Theme,
            style: &renderer::Style,
            layout: Layout<'_>,
            cursor: mouse::Cursor,
            viewport: &Rectangle,
        ) {
            self.inner
                .as_widget()
                .draw(tree, renderer, theme, style, layout, cursor, viewport);
        }

        fn mouse_interaction(
            &self,
            tree: &Tree,
            layout: Layout<'_>,
            cursor: mouse::Cursor,
            viewport: &Rectangle,
            renderer: &R,
        ) -> mouse::Interaction {
            self.inner
                .as_widget()
                .mouse_interaction(tree, layout, cursor, viewport, renderer)
        }

        fn overlay<'b>(
            &'b mut self,
            tree: &'b mut Tree,
            layout: Layout<'b>,
            renderer: &R,
            viewport: &Rectangle,
            translation: Vector,
        ) -> Option<overlay::Element<'b, Message, iced::Theme, R>> {
            self.inner
                .as_widget_mut()
                .overlay(tree, layout, renderer, viewport, translation)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(c: &str, modifiers: keyboard::Modifiers) -> Event {
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Character(c.into()),
            modified_key: keyboard::Key::Character(c.into()),
            physical_key: keyboard::key::Physical::Unidentified(
                keyboard::key::NativeCode::Unidentified,
            ),
            location: keyboard::Location::Standard,
            modifiers,
            text: Some(c.into()),
            repeat: false,
        })
    }

    #[test]
    fn switching_modes_tracks_practice_visibility() {
        // App::boot() 은 쓰지 않는다: 테스트 실행 중엔 args().nth(1) 이 테스트 이름이라 파일로 열린다
        let (writer, _) = Writer::with_path(None);
        let mut app = App::new(writer);
        assert_eq!(app.mode, Mode::Write);
        assert!(
            !app.practice.visible,
            "앱은 집중 글쓰기로 시작하므로 영어 공부는 숨어 있다"
        );
        let _ = app.update(Message::Switch(Mode::Practice));
        assert!(app.mode == Mode::Practice && app.practice.visible);
        assert_eq!(app.title(window::Id::unique()), "영어 공부");
        let _ = app.update(Message::Switch(Mode::Write));
        assert!(app.mode == Mode::Write && !app.practice.visible);
    }

    #[test]
    fn settings_window_closes_alone_and_mutes_mode_keys() {
        let (writer, _) = Writer::with_path(None);
        let mut app = App::new(writer);
        let _ = app.update(Message::Switch(Mode::Practice));
        let _ = app.update(Message::Practice(practice::Msg::ToggleSettings));
        let id = app.settings.expect("설정 버튼은 설정 창을 연다");
        assert_eq!(app.pane, Pane::Practice, "지금 모드의 분류로 연다");
        let _ = app.update(Message::OpenSettings);
        assert_eq!(app.settings, Some(id), "이미 열려 있으면 새로 열지 않는다");
        assert_eq!(app.title(id), "설정");

        let _ = app.update(Message::Focused(id));
        assert!(
            app.settings_focused,
            "설정 창에선 Tab · Esc 가 모드로 가지 않는다"
        );
        // 다른 창의 Esc 는 설정 창을 닫지 않는다
        let _ = app.update(Message::CloseKey(window::Id::unique()));
        assert_eq!(app.settings, Some(id));
        // 설정 창 닫기는 저장 확인 없이 그 창만 (메인 창은 그대로, 모드도 그대로)
        let _ = app.update(Message::CloseRequested(id));
        assert!(app.settings.is_none() && !app.settings_focused && app.mode == Mode::Practice);
    }

    #[test]
    fn command_keys_switch_modes_and_never_type_text() {
        use keyboard::Modifiers;
        let to = |e: Event| app_keys(e, event::Status::Ignored, window::Id::unique());
        assert!(matches!(
            to(press("1", Modifiers::COMMAND)),
            Some(Message::Switch(Mode::Write))
        ));
        assert!(matches!(
            to(press("2", Modifiers::COMMAND)),
            Some(Message::Switch(Mode::Practice))
        ));
        assert!(
            to(press("2", Modifiers::empty())).is_none(),
            "⌘ 없는 2 는 그냥 글자"
        );
        // ⌘ 조합은 글자만 지우고 키는 남긴다. ⌘ 없는 입력은 건드리지 않는다
        let Some(Event::Keyboard(keyboard::Event::KeyPressed { text, key, .. })) =
            command_keys::strip(&press("1", Modifiers::COMMAND))
        else {
            panic!("⌘1 은 바뀌어야 한다")
        };
        assert_eq!((text, key), (None, keyboard::Key::Character("1".into())));
        assert!(command_keys::strip(&press("a", Modifiers::SHIFT)).is_none());
    }
}
