// iamWriter — 집중 글쓰기 + 영어 공부 (iced)
// 한 창에 두 모드. ⌘1 집중 글쓰기 · ⌘2 영어 공부. 설계: docs/TYPINGENG_PORT.md
mod practice;
mod theme;
mod writer;

use iced::{
    Border, Color, Element, Event, Length, Subscription, Task, event, keyboard, system,
    widget::{button, container, row, stack, text},
    window,
};
use practice::Practice;
use theme::{TITLEBAR, UI_FONT};
use writer::Writer;

fn main() -> iced::Result {
    iced::application(App::boot, App::update, App::view)
        .title(App::title)
        .theme(App::theme)
        .default_font(UI_FONT)
        .subscription(App::subscription)
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

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    /// 집중 글쓰기: 방해 없는 마크다운 편집기
    Write,
    /// 영어 공부: 따라치기 + 영작 첨삭 (typingEng)
    Practice,
}

/// 앱 셸: 두 모드의 상태를 들고 메시지를 감싸서 전달만 한다.
struct App {
    mode: Mode,
    writer: Writer,
    practice: Practice,
}

#[derive(Debug, Clone)]
enum Message {
    Writer(writer::Message),
    Practice(practice::Msg),
    Switch(Mode),
}

impl App {
    fn boot() -> (Self, Task<Message>) {
        let (writer, task) = Writer::boot();
        (Self::new(writer), task.map(Message::Writer))
    }

    fn new(writer: Writer) -> Self {
        Self {
            mode: Mode::Write,
            writer,
            practice: Practice::new(),
        }
    }

    fn title(&self) -> String {
        match self.mode {
            Mode::Write => self.writer.title(),
            Mode::Practice => self.practice.title(),
        }
    }

    /// 테마는 하나: 집중 글쓰기 설정의 테마를 두 모드가 같이 쓴다.
    fn theme(&self) -> iced::Theme {
        self.writer.theme()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Writer(m) => self.writer.update(m).map(Message::Writer),
            // 숨어 있는 동안 도착한 AI·유튜브 결과도 여기로 온다. 포커스·재생은 Practice 가 visible 일 때만 한다.
            Message::Practice(m) => self.practice.update(m).map(Message::Practice),
            Message::Switch(mode) => self.switch(mode),
        }
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

    fn view(&self) -> Element<'_, Message> {
        let body = match self.mode {
            Mode::Write => self.writer.view().map(Message::Writer),
            Mode::Practice => self
                .practice
                .view(self.writer.palette())
                .map(Message::Practice),
        };
        // 모드 전환 버튼은 타이틀바 오른쪽에 겹쳐 그린다. 버튼 밖의 타이틀바는 아래 층이 그대로 받아 창 끌기가 된다.
        let content = stack![body, self.mode_switch()];
        Element::new(command_keys::StripCommandText {
            inner: content.into(),
        })
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
        // 키보드 구독은 보이는 모드 하나만: Tab 이 글쓰기에선 포커스 이동, 영어 공부에선 건너뛰기라 둘 다 켜면 함께 동작한다
        let mode = match self.mode {
            Mode::Write => self.writer.subscription().map(Message::Writer),
            Mode::Practice => self.practice.subscription().map(Message::Practice),
        };
        Subscription::batch([
            mode,
            event::listen_with(switch_keys),
            // 모드와 상관없이 항상. exit_on_close_request(false) 라서 이게 없으면 창이 닫히지 않는다.
            // 영어 공부 모드에서 닫아도 저장 안 된 글을 확인한다.
            // map 의 클로저는 아무것도 캡처하면 안 된다(iced 가 컴파일 단계에서 검사).
            window::close_requests().map(|id| Message::Writer(writer::Message::CloseRequested(id))),
            system::theme_changes().map(|m| Message::Writer(writer::Message::SystemMode(m))),
        ])
    }
}

/// ⌘1 집중 글쓰기 · ⌘2 영어 공부. 한글 자판에서도 물리 키 기준으로 읽는다.
fn switch_keys(event: Event, _status: event::Status, _window: window::Id) -> Option<Message> {
    let Event::Keyboard(keyboard::Event::KeyPressed {
        key,
        modifiers,
        physical_key,
        ..
    }) = event
    else {
        return None;
    };
    if !modifiers.command() {
        return None;
    }
    match key.to_latin(physical_key) {
        Some('1') => Some(Message::Switch(Mode::Write)),
        Some('2') => Some(Message::Switch(Mode::Practice)),
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
        assert_eq!(app.title(), "영어 공부");
        let _ = app.update(Message::Switch(Mode::Write));
        assert!(app.mode == Mode::Write && !app.practice.visible);
    }

    #[test]
    fn command_keys_switch_modes_and_never_type_text() {
        use keyboard::Modifiers;
        let to = |e: Event| switch_keys(e, event::Status::Ignored, window::Id::unique());
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
