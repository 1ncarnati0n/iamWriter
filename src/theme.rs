// 두 모드(집중 글쓰기 · 영어 공부)가 함께 쓰는 테마 · 팔레트 · UI 상수
use iced::{Color, Font};
use serde::{Deserialize, Serialize};

pub(crate) const TITLEBAR: f32 = 30.0; // macOS 신호등 버튼 높이만큼
/// 모든 Mac 에 있는 한글 글꼴. UI 기본 글꼴로 쓰면 한글/영문 기준선과 높이가 섞이지 않는다.
pub(crate) const UI_FONT: Font = Font::with_name("Apple SD Gothic Neo");

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub(crate) enum Theme {
    Auto,
    Light,
    Sepia,
    Dark,
    Night,
    TokyoNight,
    TokyoLight,
    /// Anthropic 브랜드 색: 아이보리 #faf9f5 · 잉크 #141413 · 주황 #d97757
    Anthropic,
    AnthropicDark,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Palette {
    pub(crate) bg: Color,
    pub(crate) panel: Color,
    pub(crate) fg: Color,
    pub(crate) strong: Color,
    /// 포커스 모드에서 흐려진 글자. 일부러 아주 옅다(읽히지 않아도 됨).
    pub(crate) dim: Color,
    /// 읽혀야 하는 보조 글자(설명·라벨). 영어 공부 모드가 쓴다.
    pub(crate) muted: Color,
    pub(crate) accent: Color,
    pub(crate) code: Color,
    /// 따라치기 맞음 / 틀림
    pub(crate) ok: Color,
    pub(crate) bad: Color,
}

impl Palette {
    /// 아주 옅은 구분선·테두리
    pub(crate) fn hair(self) -> Color {
        Color { a: 0.1, ..self.fg }
    }
}

impl Theme {
    pub(crate) const ALL: [(Theme, &'static str); 9] = [
        (Theme::Auto, "자동"),
        (Theme::Light, "라이트"),
        (Theme::Sepia, "세피아"),
        (Theme::Dark, "다크"),
        (Theme::Night, "나이트"),
        (Theme::TokyoNight, "Tokyo Night"),
        (Theme::TokyoLight, "Tokyo Night Light"),
        (Theme::Anthropic, "Anthropic"),
        (Theme::AnthropicDark, "Anthropic 다크"),
    ];

    pub(crate) fn resolve(self, system_dark: bool) -> Theme {
        match self {
            Theme::Auto if system_dark => Theme::Dark,
            Theme::Auto => Theme::Light,
            t => t,
        }
    }

    pub(crate) fn next(self) -> Self {
        let i = Self::ALL.iter().position(|(t, _)| *t == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()].0
    }

    pub(crate) fn palette(self) -> Palette {
        let c = Color::from_rgb8;
        let g = |v| Color::from_rgb8(v, v, v);
        // 맞음/틀림: 밝은 테마와 어두운 테마가 각각 같은 값을 쓴다(대비는 아래 테스트가 확인)
        let (light_ok, light_bad) = (c(30, 135, 75), c(205, 50, 60));
        let (dark_ok, dark_bad) = (c(120, 200, 140), c(240, 110, 120));
        match self {
            Theme::Light | Theme::Auto => Palette {
                bg: c(247, 247, 245),
                panel: c(255, 255, 255),
                fg: g(45),
                strong: g(0),
                dim: g(185),
                muted: g(140),
                accent: c(20, 110, 230),
                code: c(170, 60, 120),
                ok: light_ok,
                bad: light_bad,
            },
            Theme::Sepia => Palette {
                bg: c(244, 236, 216),
                panel: c(250, 244, 230),
                fg: c(91, 70, 54),
                strong: c(45, 30, 20),
                dim: c(195, 175, 145),
                muted: c(140, 115, 90),
                accent: c(185, 95, 40),
                code: c(140, 90, 150),
                ok: light_ok,
                bad: light_bad,
            },
            Theme::Dark => Palette {
                bg: c(26, 26, 28),
                panel: c(38, 38, 42),
                fg: g(215),
                strong: g(250),
                dim: g(85),
                muted: g(120),
                accent: c(90, 160, 255),
                code: c(220, 140, 190),
                ok: dark_ok,
                bad: dark_bad,
            },
            Theme::Night => Palette {
                bg: c(10, 12, 18),
                panel: c(22, 26, 36),
                fg: c(165, 175, 190),
                strong: c(230, 235, 245),
                dim: c(55, 60, 75),
                muted: c(110, 118, 135),
                accent: c(110, 200, 165),
                code: c(200, 160, 230),
                ok: dark_ok,
                bad: dark_bad,
            },
            // Tokyo Night 팔레트 (folke/tokyonight). 보조 글자는 대비 3:1 을 넘기려고 원래 comment 색보다 밝게
            Theme::TokyoNight => Palette {
                bg: c(26, 27, 38),        // #1a1b26
                panel: c(36, 40, 59),     // #24283b
                fg: c(169, 177, 214),     // #a9b1d6
                strong: c(192, 202, 245), // #c0caf5
                dim: c(59, 66, 97),       // #3b4261
                muted: c(115, 122, 162),  // #737aa2
                accent: c(122, 162, 247), // #7aa2f7
                code: c(187, 154, 247),   // #bb9af7
                ok: c(158, 206, 106),     // #9ece6a
                bad: c(247, 118, 142),    // #f7768e
            },
            Theme::TokyoLight => Palette {
                bg: c(213, 214, 219),    // #d5d6db
                panel: c(233, 233, 237), // #e9e9ed
                fg: c(52, 59, 88),       // #343b58
                strong: c(15, 15, 20),
                dim: c(150, 153, 163), // #9699a3
                muted: c(90, 94, 110),
                accent: c(52, 84, 138), // #34548a
                code: c(90, 74, 120),   // #5a4a78
                ok: c(72, 94, 48),      // #485e30
                bad: c(140, 67, 81),    // #8c4351
            },
            Theme::Anthropic => Palette {
                bg: c(250, 249, 245), // #faf9f5
                panel: c(240, 238, 230),
                fg: c(61, 61, 58),
                strong: c(20, 20, 19), // #141413
                dim: c(176, 174, 165), // #b0aea5
                muted: c(120, 118, 110),
                accent: c(201, 100, 66), // #d97757 을 조금 진하게 (흰 바탕 버튼 글자 대비)
                code: c(70, 110, 160),   // 파랑 #6a9bcc 계열
                ok: c(95, 115, 70),      // 초록 #788c5d 계열
                bad: c(190, 60, 50),
            },
            Theme::AnthropicDark => Palette {
                bg: c(20, 20, 19), // #141413
                panel: c(34, 33, 30),
                fg: c(232, 230, 220),     // #e8e6dc
                strong: c(250, 249, 245), // #faf9f5
                dim: c(74, 73, 69),
                muted: c(176, 174, 165), // #b0aea5
                accent: c(217, 119, 87), // #d97757
                code: c(106, 155, 204),  // #6a9bcc
                ok: c(150, 172, 120),
                bad: c(232, 128, 110),
            },
        }
    }
}

#[test]
fn every_theme_keeps_practice_text_readable() {
    // WCAG 대비 비율. 본문은 4.5:1, 보조 글자·맞음/틀림은 3:1 이상. dim 은 일부러 옅으므로 검사하지 않는다.
    fn luminance(c: Color) -> f32 {
        let lin = |v: f32| {
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
    }
    let ratio = |a: Color, b: Color| {
        let (x, y) = (luminance(a), luminance(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    };
    for (theme, name) in Theme::ALL {
        let p = theme.palette();
        assert!(ratio(p.fg, p.bg) >= 4.5, "{name} fg");
        for (color, what) in [(p.muted, "muted"), (p.ok, "ok"), (p.bad, "bad")] {
            assert!(
                ratio(color, p.bg) >= 3.0,
                "{name} {what}: {:.2}",
                ratio(color, p.bg)
            );
        }
    }
}
