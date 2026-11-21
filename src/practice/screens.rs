// 영어 공부 화면: 전체 틀(타이틀바 · 사이드바 자리 · 상태줄), 따라치기, 직접 쓰기, 첨삭 카드
use super::*;
use iced::widget::{column, rich_text, row};

impl Practice {
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
    pub(super) fn status_bar(&self, p: Palette) -> Element<'_, Msg> {
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

    pub(super) fn typing_view(&self, p: Palette) -> Element<'_, Msg> {
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
    pub(super) fn writing_view(&self, p: Palette) -> Element<'_, Msg> {
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
    pub(super) fn review_card<'a>(&'a self, p: Palette, r: &'a Review) -> Element<'a, Msg> {
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
