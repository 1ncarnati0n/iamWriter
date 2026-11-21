// 설정 창의 "영어 공부": AI 엔진 · 모델 · API 키 · 유튜브 · 글꼴
use super::*;
use iced::widget::{column, row};

impl Practice {
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
}
