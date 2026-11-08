use super::*;

#[test]
fn atomic_save_preserves_original_on_failure() {
    let dir = std::env::temp_dir().join(format!("iamwriter-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("note.md");
    std::fs::write(&path, "원본").unwrap();
    atomic_save(&path, "변경").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "변경");
    assert!(atomic_save(&dir, "실패").is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "변경");
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn search_uses_utf8_columns() {
    let hits = search_matches("한글 test\r\n다시 한글", "한글");
    assert_eq!(hits.len(), 2);
    assert_eq!(
        hits[1].position,
        text_editor::Position {
            line: 1,
            column: 13
        }
    );
    assert_eq!(
        hits[1].selection,
        Some(text_editor::Position { line: 1, column: 7 })
    );
    assert!(search_matches("abc", "").is_empty());
    assert!(search_matches("abc", "z").is_empty());
}

#[test]
fn font_search_matches_korean_alias() {
    assert!(font_matches("Apple SD Gothic Neo", "고딕"));
    assert!(font_matches("Menlo", "menlo"));
    assert!(!font_matches("Menlo", "명조"));
}

#[test]
fn startup_failure_has_no_save_target() {
    let (writer, _) = Writer::with_path(Some(PathBuf::from("/no-such-iamwriter-folder/note.md")));
    assert!(writer.path.is_none());
    assert!(writer.notice.is_some());
    assert!(!writer.dirty);
}

#[test]
fn undo_groups_deletions_and_breaks_after_cursor_move() {
    use text_editor::{Action, Edit, Motion};
    let (mut writer, _) = Writer::with_path(None);
    writer.set_text("abc");
    writer.content.perform(Action::Move(Motion::DocumentEnd));
    writer.edit(Action::Edit(Edit::Backspace));
    writer.edit(Action::Edit(Edit::Backspace));
    assert_eq!(writer.undo.len(), 1);
    writer.undo_redo(false);
    assert_eq!(writer.content.text().trim_end(), "abc");
    writer.undo_redo(true);
    assert_eq!(writer.content.text().trim_end(), "a");
    writer.edit(Action::Move(Motion::DocumentEnd));
    writer.edit(Action::Edit(Edit::Insert('한')));
    writer.edit(Action::Move(Motion::Left));
    writer.edit(Action::Edit(Edit::Insert('글')));
    writer.undo_redo(false);
    assert_eq!(writer.content.text().trim_end(), "a한");
}

#[test]
fn search_wraps_and_does_not_edit_document() {
    let (mut writer, _) = Writer::with_path(None);
    writer.set_text("한글\n다시 한글");
    let before = writer.content.text();
    writer.show_find = true;
    writer.find_query = "한글".into();
    writer.refresh_matches();
    writer.select_match(false, false);
    assert_eq!(writer.content.selection().as_deref(), Some("한글"));
    writer.select_match(true, true);
    assert_eq!(writer.match_index, 1);
    writer.select_match(false, true);
    assert_eq!(writer.match_index, 0);
    assert_eq!(writer.content.text(), before);
    assert!(writer.undo.is_empty());
    assert!(!writer.dirty);
}

#[test]
fn settings_and_search_shortcuts_do_not_undo_body() {
    let (mut writer, _) = Writer::with_path(None);
    writer.edit(text_editor::Action::Edit(text_editor::Edit::Insert('한')));
    let before = writer.content.text();
    writer.show_settings = true;
    let _ = writer.update(Message::Shortcut("z".into(), false));
    assert_eq!(writer.content.text(), before);
    writer.show_settings = false;
    writer.show_find = true;
    let _ = writer.update(Message::Shortcut("z".into(), false));
    assert_eq!(writer.content.text(), before);
    let _ = writer.update(Message::Escape);
    assert!(!writer.show_find);
    writer.undo_redo(false);
    assert!(writer.content.text().trim().is_empty());
}

#[test]
fn measured_width_scales_with_font_size() {
    let font = Font::with_name("Menlo");
    let small = zero_width(font, 12.0);
    let big = zero_width(font, 24.0);
    assert!(small > 1.0);
    assert!((big / small - 2.0).abs() < 0.1);
}

#[cfg(unix)]
#[test]
fn atomic_save_preserves_symlink_and_mode_and_rejects_readonly() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let dir = std::env::temp_dir().join(format!("iamwriter-link-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("note.md");
    let link = dir.join("link.md");
    std::fs::write(&path, "원본").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
    symlink(&path, &link).unwrap();
    atomic_save(&link, "수정").unwrap();
    assert!(link.is_symlink());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "수정");
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o640
    );
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o400)).unwrap();
    assert!(atomic_save(&path, "덮어쓰기").is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "수정");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn new_document_clears_path_and_history() {
    let (mut writer, _) = Writer::with_path(None);
    writer.path = Some(PathBuf::from("previous.md"));
    writer.find_query = "이전".into();
    let _ = writer.update(Message::Shortcut("n".into(), false));
    assert!(writer.path.is_none());
    assert!(writer.content.text().is_empty());
    assert!(writer.undo.is_empty());
    assert!(!writer.dirty);
}

#[test]
fn selected_text_replacement_undo_restores_selection() {
    let (mut writer, _) = Writer::with_path(None);
    writer.set_text("한글 문서");
    writer
        .content
        .move_to(search_matches("한글 문서", "한글")[0]);
    writer.edit(text_editor::Action::Edit(text_editor::Edit::Paste(
        std::sync::Arc::new("새 글".into()),
    )));
    writer.undo_redo(false);
    assert_eq!(writer.content.text().trim_end(), "한글 문서");
    assert_eq!(writer.content.selection().as_deref(), Some("한글"));
}

#[test]
fn search_highlight_keeps_markdown_and_marks_current_result() {
    use iced::advanced::text::Highlighter;
    let p = Theme::Light.palette();
    let faces = Faces {
        regular: Font::DEFAULT,
        bold: Font {
            weight: font::Weight::Bold,
            ..Font::DEFAULT
        },
        italic: Font::DEFAULT,
        bold_italic: Font::DEFAULT,
    };
    let settings = HlSettings {
        p,
        faces,
        focus: None,
        found: Some(search_matches("# 한글 제목", "한글")[0]),
    };
    let mut highlighter = Markdown::new(&settings);
    let spans: Vec<_> = highlighter.highlight_line("# 한글 제목").collect();
    assert!(
        spans
            .iter()
            .any(|(range, style)| range == &(2..8) && style.color == p.accent)
    );
    assert!(
        spans
            .iter()
            .any(|(range, style)| range.end == "# 한글 제목".len() && style.color == p.strong)
    );
}

#[test]
#[ignore = "manual comparison of view preparation, not end-to-end typing latency"]
fn compare_focus_preparation() {
    let text = "한글 English 문단입니다.\n두 번째 줄\n\n".repeat(7000);
    let content = text_editor::Content::with_text(&text);
    let at = 10500;
    let start = Instant::now();
    for _ in 0..100 {
        let lines: Vec<_> = content.lines().map(|l| l.text.into_owned()).collect();
        std::hint::black_box(lines);
    }
    let before = start.elapsed();
    let start = Instant::now();
    for _ in 0..100 {
        assert_eq!(std::hint::black_box(para_lines(&content, at)), (at, at + 1));
    }
    eprintln!(
        "21,001 lines / 100 iterations: full copy {:?}, local paragraph {:?}",
        before,
        start.elapsed()
    );
}

fn key_press(key: keyboard::Key, modifiers: keyboard::Modifiers) -> text_editor::KeyPress {
    text_editor::KeyPress {
        modified_key: key.clone(),
        key,
        physical_key: keyboard::key::Physical::Unidentified(
            keyboard::key::NativeCode::Unidentified,
        ),
        modifiers,
        text: None,
        status: text_editor::Status::Focused { is_hovered: false },
    }
}

#[test]
fn mac_delete_bindings_remove_to_line_or_word_boundary_and_undo() {
    use keyboard::{Key, Modifiers, key::Named};
    use text_editor::{Binding, Motion};
    for (key, modifiers, motion, expected) in [
        (
            Named::Backspace,
            Modifiers::COMMAND,
            Motion::Home,
            "\n다음 줄",
        ),
        (
            Named::Backspace,
            Modifiers::ALT,
            Motion::WordLeft,
            "hello \n다음 줄",
        ),
    ] {
        let Some(Binding::Custom(message)) = editor_binding(key_press(Key::Named(key), modifiers))
        else {
            panic!("missing delete binding")
        };
        assert!(matches!(&message, Message::DeleteTo(m) if *m == motion));
        let (mut writer, _) = Writer::with_path(None);
        writer.set_text("hello world\n다음 줄");
        writer.content.move_to(text_editor::Cursor {
            position: text_editor::Position {
                line: 0,
                column: 11,
            },
            selection: None,
        });
        let _ = writer.update(message);
        assert_eq!(writer.content.text().trim_end(), expected);
        writer.undo_redo(false);
        assert_eq!(writer.content.text().trim_end(), "hello world\n다음 줄");
    }
}

#[test]
fn formatting_toggles_selection_and_keeps_one_undo_step() {
    let (mut writer, _) = Writer::with_path(None);
    writer.set_text("한글 문장");
    writer
        .content
        .move_to(search_matches("한글 문장", "한글")[0]);
    writer.format_selection("**");
    assert_eq!(writer.content.text().trim_end(), "**한글** 문장");
    assert_eq!(writer.content.selection().as_deref(), Some("한글"));
    writer.format_selection("**");
    assert_eq!(writer.content.text().trim_end(), "한글 문장");
    writer.undo_redo(false);
    assert_eq!(writer.content.text().trim_end(), "**한글** 문장");
    writer.undo_redo(false);
    assert_eq!(writer.content.text().trim_end(), "한글 문장");
    writer.content.move_to(text_editor::Cursor {
        position: text_editor::Position { line: 0, column: 0 },
        selection: None,
    });
    writer.format_selection("*");
    assert_eq!(writer.content.cursor().position.column, 1);
}

#[test]
fn markdown_combined_emphasis_and_underscore_bold() {
    for (input, expected) in [
        ("***한글***", Kind::BoldItalic),
        ("___한글___", Kind::BoldItalic),
        ("__한글__", Kind::Bold),
    ] {
        let spans = inline_spans(input);
        assert_eq!(spans[1].1, expected);
        assert_eq!(&input[spans[1].0.clone()], "한글");
    }
    assert!(
        inline_spans("\\*기호\\*")
            .iter()
            .all(|(_, k)| *k == Kind::Plain)
    );
}

#[test]
fn korean_italic_glyphs_use_synthetic_slant_without_changing_family() {
    use iced::advanced::graphics::text::{
        cosmic_text::{Buffer, CacheKeyFlags, Metrics, Shaping},
        to_attributes,
    };
    let faces = faces_of("Apple SD Gothic Neo");
    let attrs = to_attributes(faces.italic);
    assert!(attrs.cache_key_flags.contains(CacheKeyFlags::FAKE_ITALIC));
    assert_eq!(faces.italic.family, faces.regular.family);
    let mut system = font_system().write().unwrap();
    let system = system.raw();
    let mut buffer = Buffer::new(system, Metrics::new(18.0, 28.0));
    buffer.set_text(system, "한글 Italic", &attrs, Shaping::Advanced, None);
    buffer.shape_until_scroll(system, false);
    let glyphs: Vec<_> = buffer.layout_runs().flat_map(|r| r.glyphs.iter()).collect();
    assert!(!glyphs.is_empty());
    assert!(
        glyphs
            .iter()
            .all(|g| g.glyph_id != 0 && g.cache_key_flags.contains(CacheKeyFlags::FAKE_ITALIC))
    );
    let italic_key = glyphs[0].physical((0.0, 0.0), 1.0).cache_key;
    buffer.set_text(
        system,
        "한글 Italic",
        &to_attributes(faces.regular),
        Shaping::Advanced,
        None,
    );
    buffer.shape_until_scroll(system, false);
    let regular_key = buffer.layout_runs().next().unwrap().glyphs[0]
        .physical((0.0, 0.0), 1.0)
        .cache_key;
    let mut cache = iced::advanced::graphics::text::cosmic_text::SwashCache::new();
    let italic_pixels = cache
        .get_image(system, italic_key)
        .as_ref()
        .unwrap()
        .data
        .clone();
    let regular_pixels = cache
        .get_image(system, regular_key)
        .as_ref()
        .unwrap()
        .data
        .clone();
    assert_ne!(italic_pixels, regular_pixels);
}

#[test]
fn bold_hangul_and_latin_use_bold_faces() {
    use iced::advanced::graphics::text::{
        cosmic_text::{Buffer, Metrics, Shaping},
        to_attributes,
    };
    let faces = faces_of("Menlo");
    let mut system = font_system().write().unwrap();
    let system = system.raw();
    let mut buffer = Buffer::new(system, Metrics::new(18.0, 28.0));
    buffer.set_text(
        system,
        "한글 Bold",
        &to_attributes(faces.bold),
        Shaping::Advanced,
        None,
    );
    buffer.shape_until_scroll(system, false);
    for glyph in buffer.layout_runs().flat_map(|r| r.glyphs.iter()) {
        assert!(system.db().face(glyph.font_id).unwrap().weight.0 >= 600);
    }
}

#[test]
fn single_weight_korean_font_has_visible_bold_fallback() {
    let faces = faces_of("AppleMyungjo");
    assert!(matches!(
        faces.bold.weight,
        font::Weight::Semibold | font::Weight::Bold | font::Weight::ExtraBold | font::Weight::Black
    ));
}
