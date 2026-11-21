// 처음 화면: 왼쪽 사이드바와 기능별 화면(복습 · 영상 · AI 문장 · 직접 쓰기 · 보관함 · 기록)
use super::*;
use iced::widget::{column, row};

impl Practice {
    /// 처음 화면의 왼쪽 사이드바: 기능 목록 + 아래에 설정.
    pub(super) fn sidebar(&self, p: Palette) -> Element<'_, Msg> {
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
    pub(super) fn setup_view(&self, p: Palette) -> Element<'_, Msg> {
        match self.section {
            Section::Review => self.review_section(p),
            Section::YouTube => self.youtube_section(p),
            Section::Ai => self.ai_section(p),
            Section::Write => self.write_section(p),
            Section::Library => self.library_section(p),
            Section::Records => self.records_section(p),
        }
    }

    pub(super) fn review_section(&self, p: Palette) -> Element<'_, Msg> {
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
    pub(super) fn youtube_section(&self, p: Palette) -> Element<'_, Msg> {
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

    pub(super) fn ai_section(&self, p: Palette) -> Element<'_, Msg> {
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
    pub(super) fn write_section(&self, p: Palette) -> Element<'_, Msg> {
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

    pub(super) fn library_section(&self, p: Palette) -> Element<'_, Msg> {
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
    pub(super) fn library_rows(&self, p: Palette, videos: bool, lessons: bool) -> Element<'_, Msg> {
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
    pub(super) fn records_section(&self, p: Palette) -> Element<'_, Msg> {
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
}
