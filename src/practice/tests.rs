// 영어 공부 모드 테스트 (실제 저장소는 프로세스별 임시 폴더라 실제 데이터를 건드리지 않는다)
use super::*;

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
