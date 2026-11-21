// 영어 공부 화면들이 같이 쓰는 버튼 스타일 · 카드 · 섹션 같은 작은 조각
use super::*;
use iced::widget::{column, row};

pub(super) type ButtonStyle = Box<dyn Fn(&iced::Theme, button::Status) -> button::Style>;

pub(super) fn primary(p: Palette) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
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
pub(super) fn pill(p: Palette, on: bool) -> ButtonStyle {
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
pub(super) fn quiet(p: Palette) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, st| button::Style {
        background: matches!(st, button::Status::Hovered | button::Status::Pressed)
            .then(|| Color { a: 0.1, ..p.accent }.into()),
        text_color: p.fg,
        border: Border::default().rounded(6),
        ..Default::default()
    }
}

pub(super) fn field(p: Palette) -> impl Fn(&iced::Theme, text_input::Status) -> text_input::Style {
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

/// 떠 있는 카드: 패널 바탕, 가는 테두리, 부드러운 그림자.
pub(super) fn card<'a>(p: Palette, content: impl Into<Element<'a, Msg>>) -> Element<'a, Msg> {
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
pub(super) fn section<'a>(
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
pub(super) fn library_row(
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
pub(super) fn side(p: Palette, on: bool) -> ButtonStyle {
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
pub(super) fn page_title<'a>(p: Palette, title: &'a str, desc: &'a str) -> Element<'a, Msg> {
    column![
        text(title).size(28).color(p.strong),
        text(desc).size(14).color(p.muted)
    ]
    .spacing(4)
    .into()
}

/// 이 기능이 어떻게 동작하는지 짧은 안내 목록
pub(super) fn guide<'a>(p: Palette, lines: &[&'a str]) -> Element<'a, Msg> {
    Column::with_children(
        lines
            .iter()
            .map(|&l| text(format!("· {l}")).size(13).color(p.muted).into()),
    )
    .spacing(6)
    .into()
}

pub(super) fn toggle(p: Palette, label: &str, on: bool, msg: Msg) -> Element<'_, Msg> {
    button(text(label).size(13))
        .style(pill(p, on))
        .on_press(msg)
        .padding([5, 14])
        .into()
}

pub(super) fn labeled<'a>(
    p: Palette,
    label: &'a str,
    content: Element<'a, Msg>,
) -> Element<'a, Msg> {
    row![text(label).size(13).width(48).color(p.muted), content]
        .spacing(12)
        .align_y(Center)
        .into()
}
