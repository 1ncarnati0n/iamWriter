// iamWriter — 방해 없는 마크다운 글쓰기 도구 (egui)
// 단축키: ⌘O 열기 · ⌘S 저장 · ⇧⌘S 다른 이름으로 · ⌘, 설정
//         ⇧⌘F 포커스 · ⇧⌘T 타자기 · ⇧⌘D 테마 순환
use eframe::egui::{
    self, text::{CCursor, LayoutJob}, Align, Color32, FontData, FontFamily, FontId, Key, KeyboardShortcut,
    Modifiers, TextFormat,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    ops::Range,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

const MENLO: &str = "/System/Library/Fonts/Menlo.ttc";
const KOREAN: &str = "/System/Library/Fonts/AppleSDGothicNeo.ttc";
const TITLEBAR: f32 = 30.0; // macOS 신호등 버튼 높이만큼

fn main() -> eframe::Result {
    let path = std::env::args().nth(1).map(PathBuf::from);
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 760.0])
            .with_fullsize_content_view(true) // 본문 배경이 타이틀바 뒤까지
            .with_titlebar_shown(false)
            .with_title_shown(false),
        ..Default::default()
    };
    eframe::run_native("iamWriter", opts, Box::new(|cc| {
        let settings: Settings = cc.storage.and_then(|s| eframe::get_value(s, eframe::APP_KEY)).unwrap_or_default();
        Ok(Box::new(Writer::new(path, settings, &cc.egui_ctx)))
    }))
}

// ───────────────────────── 설정 / 테마 ─────────────────────────

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq)]
enum Theme { Auto, Light, Sepia, Dark, Night }

#[derive(Clone, Copy)]
struct Palette { bg: Color32, fg: Color32, strong: Color32, dim: Color32, accent: Color32, code_bg: Color32 }

impl Theme {
    const ALL: [(Theme, &'static str); 5] = [
        (Theme::Auto, "자동"), (Theme::Light, "라이트"), (Theme::Sepia, "세피아"), (Theme::Dark, "다크"), (Theme::Night, "나이트"),
    ];

    /// Auto 는 시스템 라이트/다크를 따른다
    fn resolve(self, ctx: &egui::Context) -> Theme {
        match (self, ctx.system_theme()) {
            (Theme::Auto, Some(egui::Theme::Light)) => Theme::Light,
            (Theme::Auto, _) => Theme::Dark,
            (t, _) => t,
        }
    }

    fn is_dark(self) -> bool { matches!(self, Theme::Dark | Theme::Night) }

    fn next(self) -> Self {
        let i = Self::ALL.iter().position(|(t, _)| *t == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()].0
    }

    fn palette(self) -> Palette {
        let c = Color32::from_rgb;
        let g = Color32::from_gray;
        match self {
            Theme::Light | Theme::Auto => Palette { bg: c(247, 247, 245), fg: g(45), strong: g(0), dim: g(185), accent: c(20, 110, 230), code_bg: c(232, 232, 228) },
            Theme::Sepia => Palette { bg: c(244, 236, 216), fg: c(91, 70, 54), strong: c(45, 30, 20), dim: c(195, 175, 145), accent: c(185, 95, 40), code_bg: c(232, 221, 194) },
            Theme::Dark => Palette { bg: c(26, 26, 28), fg: g(215), strong: g(250), dim: g(85), accent: c(90, 160, 255), code_bg: g(44) },
            Theme::Night => Palette { bg: c(10, 12, 18), fg: c(165, 175, 190), strong: c(230, 235, 245), dim: c(55, 60, 75), accent: c(110, 200, 165), code_bg: c(26, 30, 40) },
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    font: Option<PathBuf>, // None = Menlo
    size: f32,
    line: f32,
    measure: u32, // 본문 폭(글자 수, '0' 기준)
    theme: Theme,
    focus: bool,
    typewriter: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { font: None, size: 18.0, line: 1.7, measure: 64, theme: Theme::Auto, focus: false, typewriter: true }
    }
}

// ───────────────────────── 글꼴 ─────────────────────────

/// 설치된 글꼴(굵기·기울임 변형 제외) 목록: (표시 이름, 경로)
fn scan_fonts() -> Vec<(String, PathBuf)> {
    const VARIANTS: [&str; 9] = ["Bold", "Italic", "Oblique", "Light", "Medium", "Semibold", "Heavy", "Black", "Thin"];
    let home = std::env::var("HOME").unwrap_or_default();
    let dirs = ["/System/Library/Fonts".into(), "/System/Library/Fonts/Supplemental".into(), "/Library/Fonts".into(), format!("{home}/Library/Fonts")];
    let mut out: Vec<_> = dirs.iter()
        .filter_map(|d| std::fs::read_dir(d).ok())
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| is_font_file(p))
        .filter_map(|p| {
            let name = p.file_stem()?.to_string_lossy().into_owned();
            (!VARIANTS.iter().any(|v| name.contains(v))).then_some((name, p))
        })
        .collect();
    out.sort_by_key(|(n, _)| n.to_lowercase());
    out
}

fn is_font_file(p: &Path) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| ["ttf", "otf", "ttc"].contains(&e.to_ascii_lowercase().as_str()))
}

/// 글꼴 파일을 한 번만 읽어 'static 으로 둔다. egui 는 Owned 데이터를 set_fonts 마다 복사하지만
/// Borrowed 는 복사하지 않는다(한글 글꼴만 55MB).
/// ponytail: 고른 글꼴 파일은 프로세스가 끝날 때까지 메모리에 남는다. 수십 개를 바꿔 볼 게 아니면 문제없음.
fn read_static(path: &Path) -> Result<&'static [u8], String> {
    static CACHE: Mutex<Option<HashMap<PathBuf, &'static [u8]>>> = Mutex::new(None);
    let mut cache = CACHE.lock().unwrap();
    let cache = cache.get_or_insert_with(HashMap::new);
    if let Some(b) = cache.get(path) { return Ok(b); }
    let bytes: &'static [u8] = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?.leak();
    cache.insert(path.to_owned(), bytes);
    Ok(bytes)
}

type Face = (PathBuf, u32); // (파일, 컬렉션 안 인덱스)

/// egui 와 같은 파서(skrifa)로 먼저 검증한다. 검증 없이 넘기면 egui 가 panic 한다.
fn load_face((path, index): &Face) -> Result<FontData, String> {
    let bytes = read_static(path)?;
    skrifa::FontRef::from_index(bytes, *index).map_err(|e| format!("{}: 읽을 수 없는 글꼴 ({e})", path.display()))?;
    Ok(FontData { index: *index, ..FontData::from_static(bytes) })
}

/// 같은 글꼴의 실제 Bold / Italic 면을 찾는다.
/// 후보: 같은 .ttc 안의 다른 면 + 같은 폴더의 "<이름>-Bold", "<이름> Italic" 같은 형제 파일.
fn find_variants(path: &Path) -> (Option<Face>, Option<Face>) {
    use skrifa::{attribute::Style, MetadataProvider as _};
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let base = stem.strip_suffix("Regular").unwrap_or(&stem).trim_end_matches(['-', ' ', '_']).to_owned();
    let siblings = path.parent().and_then(|d| std::fs::read_dir(d).ok()).into_iter().flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p != path && is_font_file(p))
        .filter(|p| {
            let s = p.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
            let rest: String = s.strip_prefix(&base.to_lowercase()).unwrap_or("x").chars().filter(|c| !"-_ ".contains(*c)).collect();
            ["bold", "italic", "oblique"].contains(&rest.as_str())
        });
    let (mut bold, mut italic): (Option<(f32, Face)>, Option<(f32, Face)>) = (None, None);
    for file in std::iter::once(path.to_owned()).chain(siblings) {
        let Ok(bytes) = read_static(&file) else { continue };
        let Ok(fr) = skrifa::raw::FileRef::new(bytes) else { continue };
        for (i, font) in fr.fonts().enumerate() {
            let Ok(font) = font else { continue };
            let a = font.attributes();
            let (w, it) = (a.weight.value(), !matches!(a.style, Style::Normal));
            let face = (file.clone(), i as u32);
            let better = |slot: &Option<(f32, Face)>, d: f32| slot.as_ref().is_none_or(|(best, _)| d < *best);
            if !it && (600.0..=800.0).contains(&w) && better(&bold, (w - 700.0).abs()) { bold = Some(((w - 700.0).abs(), face)); }
            else if it && w < 600.0 && better(&italic, (w - 400.0).abs()) { italic = Some(((w - 400.0).abs(), face)); }
        }
    }
    (bold.map(|b| b.1), italic.map(|i| i.1))
}

/// 본문 패밀리 "body" / "body-bold" / "body-italic" 를 등록한다. 각각 [본문 면, 한글 보조, egui 기본].
/// 반환: (실제 Italic 면이 있는지, 오류 메시지). 고른 글꼴을 못 읽으면 Menlo 로 되돌린다.
fn apply_fonts(ctx: &egui::Context, body: Option<&Path>) -> (bool, Option<String>) {
    let menlo = Path::new(MENLO);
    let (path, err) = match body.map(|p| load_face(&(p.to_owned(), 0)).map(|_| p)) {
        Some(Ok(p)) => (p, None),
        other => (menlo, other.and_then(Result::err)),
    };
    let (body_b, body_i) = find_variants(path);
    let (ko_b, _) = find_variants(Path::new(KOREAN));
    let real_italic = body_i.is_some();

    let mut fonts = egui::FontDefinitions::default();
    let defaults = fonts.families[&FontFamily::Monospace].clone();
    let mut add = |name: &str, face: Option<Face>, fallback: &str| -> String {
        match face.map(|f| load_face(&f)) {
            Some(Ok(d)) => { fonts.font_data.insert(name.into(), Arc::new(d)); name.into() }
            _ => fallback.into(),
        }
    };
    let body = add("body", Some((path.to_owned(), 0)), "");
    let ko = add("ko", Some((KOREAN.into(), 0)), "");
    let bold = add("body-b", body_b, &body);
    let ko_bold = add("ko-b", ko_b, &ko);
    let italic = add("body-i", body_i, &body);

    for f in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts.families.entry(f).or_default().push(ko.clone());
    }
    for (family, main, korean) in [("body", &body, &ko), ("body-bold", &bold, &ko_bold), ("body-italic", &italic, &ko)] {
        let mut v: Vec<String> = [main, korean].into_iter().filter(|n| !n.is_empty()).cloned().collect();
        v.extend(defaults.iter().cloned());
        fonts.families.insert(FontFamily::Name(family.into()), v);
    }
    ctx.set_fonts(fonts);
    (real_italic, err)
}

fn body_font(family: &str, size: f32) -> FontId { FontId::new(size, FontFamily::Name(family.into())) }

// ───────────────────────── 마크다운 강조 ─────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind { Plain, Mark, Bold, Italic, Code }

/// 한 줄 안의 `**굵게**`, `*기울임*`/`_기울임_`, `` `코드` `` 를 잘라 낸다. 닫히지 않은 기호는 그냥 글자.
fn inline_spans(s: &str) -> Vec<(Range<usize>, Kind)> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let (mut i, mut plain) = (0, 0);
    while i < b.len() {
        let (open, close, kind) = match b[i] {
            b'`' => (1, "`", Kind::Code),
            b'*' if b.get(i + 1) == Some(&b'*') => (2, "**", Kind::Bold),
            b'*' => (1, "*", Kind::Italic),
            b'_' if i == 0 || !b[i - 1].is_ascii_alphanumeric() => (1, "_", Kind::Italic),
            _ => { i += 1; continue; }
        };
        let body = i + open;
        let found = s[body..].find(close).filter(|&n| n > 0 && !s[body..].starts_with(' '));
        let Some(n) = found else { i += open; continue };
        if plain < i { out.push((plain..i, Kind::Plain)); }
        out.push((i..body, Kind::Mark));
        out.push((body..body + n, kind));
        out.push((body + n..body + n + close.len(), Kind::Mark));
        i = body + n + close.len();
        plain = i;
    }
    if plain < b.len() { out.push((plain..b.len(), Kind::Plain)); }
    out
}

/// 줄 앞 기호(제목/인용/목록)의 길이와 종류
fn line_prefix(line: &str) -> (usize, Option<&'static str>) {
    let hashes = line.bytes().take_while(|&b| b == b'#').count();
    if (1..=6).contains(&hashes) && line[hashes..].starts_with(' ') { return (hashes + 1, Some("h")); }
    if line.starts_with("> ") { return (2, Some("q")); }
    if ["- ", "* ", "+ "].iter().any(|p| line.starts_with(p)) { return (2, Some("l")); }
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    if digits > 0 && line[digits..].starts_with(". ") { return (digits + 2, Some("l")); }
    (0, None)
}

/// byte 위치를 포함하는 문단(빈 줄로 구분)의 byte 범위
fn para_range(text: &str, at: usize) -> Range<usize> {
    let start = text[..at].rfind("\n\n").map_or(0, |i| i + 2);
    let end = text[at..].find("\n\n").map_or(text.len(), |i| at + i);
    start..end
}

/// 레이아웃에 필요한 프레임별 값
#[derive(Clone, Copy)]
struct Ink { p: Palette, size: f32, line: f32, real_italic: bool }

/// `measure(s)` = 본문 글꼴로 s 의 폭(pt). 제목 기호를 접는 데 쓴다.
fn layout(text: &str, focus: Option<Range<usize>>, ink: Ink, wrap: f32, measure: &mut dyn FnMut(&str) -> f32) -> LayoutJob {
    let p = ink.p;
    let fmt = |family: &str, color, italics: bool, background| TextFormat {
        font_id: body_font(family, ink.size), color, italics, background,
        line_height: Some(ink.size * ink.line), ..Default::default()
    };
    let plain = |color| fmt("body", color, false, Color32::TRANSPARENT);
    let italic = |color| if ink.real_italic { fmt("body-italic", color, false, Color32::TRANSPARENT) } else { fmt("body", color, true, Color32::TRANSPARENT) };
    let bold = |color| fmt("body-bold", color, false, Color32::TRANSPARENT);

    let mut job = LayoutJob { wrap: egui::text::TextWrapping { max_width: wrap, ..Default::default() }, ..Default::default() };
    let mut off = 0;
    for line in text.split_inclusive('\n') {
        let r = off..off + line.len();
        off = r.end;
        let dimmed = matches!(&focus, Some(f) if r.end <= f.start || r.start > f.end);
        let (n, kind) = line_prefix(line);
        if kind == Some("h") {
            // 행잉 제목: "## " 를 폭 0 으로 접어(음수 자간) 숨기고, 본문 왼쪽 여백에 따로 그린다(paint_hanging_marks)
            let mut f = plain(Color32::TRANSPARENT);
            f.extra_letter_spacing = -measure(&line[..n]) / (n - 1) as f32;
            job.append(&line[..n], 0.0, f);
            job.append(&line[n..], 0.0, bold(if dimmed { p.dim } else { p.strong }));
            continue;
        }
        if dimmed {
            job.append(line, 0.0, plain(p.dim));
            continue;
        }
        job.append(&line[..n], 0.0, plain(if kind == Some("q") { p.dim } else { p.accent }));
        let rest = &line[n..];
        let quote = kind == Some("q");
        for (r, k) in inline_spans(rest) {
            let f = match k {
                Kind::Plain if quote => italic(p.fg),
                Kind::Plain => plain(p.fg),
                Kind::Mark => plain(p.dim),
                Kind::Bold => bold(p.strong),
                Kind::Italic => italic(p.fg),
                Kind::Code => fmt("body", p.fg, false, p.code_bg),
            };
            job.append(&rest[r], 0.0, f);
        }
    }
    if text.is_empty() || text.ends_with('\n') { job.append("", 0.0, plain(p.fg)); } // 빈 줄 커서 높이 유지
    job
}

/// 제목 줄의 "## " 를 본문 열 왼쪽 바깥에 그린다
fn paint_hanging_marks(ui: &egui::Ui, out: &egui::text_edit::TextEditOutput, text: &str, focus: &Option<Range<usize>>, ink: Ink) {
    let (mut byte, mut ch) = (0, 0);
    for line in text.split_inclusive('\n') {
        if let (n, Some("h")) = line_prefix(line) {
            let dimmed = matches!(focus, Some(f) if byte + line.len() <= f.start || byte > f.end);
            let color = if dimmed { ink.p.dim } else { ink.p.accent };
            let row = out.galley.pos_from_cursor(CCursor::new(ch)).translate(out.galley_pos.to_vec2());
            let job = LayoutJob::simple_format(line[..n].to_owned(), TextFormat {
                font_id: body_font("body", ink.size), color, line_height: Some(ink.size * ink.line), ..Default::default()
            });
            let g = ui.fonts_mut(|f| f.layout_job(job));
            let w: f32 = ui.fonts_mut(|f| line[..n].chars().map(|c| f.glyph_width(&body_font("body", ink.size), c)).sum());
            ui.painter().galley(egui::pos2(out.galley_pos.x - w, row.min.y), g, color);
        }
        byte += line.len();
        ch += line.chars().count();
    }
}

// ───────────────────────── 앱 ─────────────────────────

struct Writer {
    path: Option<PathBuf>,
    text: String,
    saved: String,
    settings: Settings,
    fonts: Vec<(String, PathBuf)>,
    loaded_font: Option<PathBuf>,
    real_italic: bool,
    font_query: String,
    show_settings: bool,
    chrome: bool,
    notice: Option<String>,
    cursor: usize, // 직전 프레임의 커서(char 인덱스)
}

impl Writer {
    fn new(path: Option<PathBuf>, mut settings: Settings, ctx: &egui::Context) -> Self {
        // 첫 프레임 전에 "body" 패밀리가 반드시 있어야 한다
        let (real_italic, notice) = apply_fonts(ctx, settings.font.as_deref());
        if notice.is_some() { settings.font = None; }
        let text = path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
        Self {
            path, saved: text.clone(), text, loaded_font: settings.font.clone(), settings, real_italic,
            fonts: scan_fonts(), font_query: String::new(), show_settings: false, chrome: true, notice, cursor: 0,
        }
    }

    fn dirty(&self) -> bool { self.text != self.saved }

    fn open(&mut self) {
        if self.dirty() && !self.confirm_discard() { return; }
        let Some(p) = md_dialog().pick_file() else { return };
        match std::fs::read_to_string(&p) {
            Ok(t) => { self.saved = t.clone(); self.text = t; self.path = Some(p); }
            Err(e) => error(&format!("열기 실패: {e}")),
        }
    }

    /// 저장 성공 여부 반환
    fn save(&mut self, save_as: bool) -> bool {
        let path = match (&self.path, save_as) {
            (Some(p), false) => p.clone(),
            _ => match md_dialog().set_file_name("Untitled.md").save_file() { Some(p) => p, None => return false },
        };
        match std::fs::write(&path, &self.text) {
            Ok(()) => { self.saved = self.text.clone(); self.path = Some(path); true }
            Err(e) => { error(&format!("저장 실패: {e}")); false }
        }
    }

    /// 저장되지 않은 변경이 있을 때: 저장/버리기/취소. 계속 진행해도 되면 true.
    fn confirm_discard(&mut self) -> bool {
        use rfd::{MessageButtons, MessageDialog, MessageDialogResult as R};
        match MessageDialog::new()
            .set_title("저장되지 않은 변경")
            .set_description("변경 사항을 저장할까요?")
            .set_buttons(MessageButtons::YesNoCancel)
            .show()
        {
            R::Yes => self.save(false),
            R::No => true,
            _ => false,
        }
    }

    fn file_name(&self) -> String {
        self.path.as_ref().and_then(|p| p.file_name()).map_or("Untitled".into(), |n| n.to_string_lossy().into_owned())
    }

    fn font_name(&self) -> String {
        self.settings.font.as_ref().and_then(|p| p.file_stem()).map_or("기본 (Menlo)".into(), |n| n.to_string_lossy().into_owned())
    }

    fn settings_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(12.0);
        ui.heading("설정");
        ui.add_space(12.0);

        ui.label("글꼴");
        egui::ComboBox::from_id_salt("font").width(240.0).height(420.0)
            .selected_text(self.font_name())
            .show_ui(ui, |ui| {
                ui.add(egui::TextEdit::singleline(&mut self.font_query).hint_text("검색…").desired_width(220.0));
                ui.separator();
                let s = &mut self.settings;
                ui.selectable_value(&mut s.font, None, "기본 (Menlo)");
                let q = self.font_query.to_lowercase();
                for (name, path) in self.fonts.iter().filter(|(n, _)| n.to_lowercase().contains(&q)) {
                    ui.selectable_value(&mut s.font, Some(path.clone()), name);
                }
            });
        ui.add_space(8.0);
        let s = &mut self.settings;
        ui.add(egui::Slider::new(&mut s.size, 12.0..=32.0).text("크기").step_by(1.0));
        ui.add(egui::Slider::new(&mut s.line, 1.2..=2.4).text("줄간격"));
        ui.add(egui::Slider::new(&mut s.measure, 40..=100).text("본문 폭 (글자)"));

        ui.add_space(12.0);
        ui.label("테마");
        ui.horizontal_wrapped(|ui| {
            for (t, name) in Theme::ALL { ui.selectable_value(&mut s.theme, t, name); }
        });

        ui.add_space(12.0);
        ui.checkbox(&mut s.focus, "포커스 모드  ⇧⌘F");
        ui.checkbox(&mut s.typewriter, "타자기 모드  ⇧⌘T");

        ui.add_space(16.0);
        if ui.button("기본값으로").clicked() { *s = Settings::default(); }
    }
}

fn md_dialog() -> rfd::FileDialog {
    rfd::FileDialog::new().add_filter("Markdown", &["md", "markdown", "txt"])
}

fn error(msg: &str) {
    rfd::MessageDialog::new().set_level(rfd::MessageLevel::Error).set_description(msg).show();
}

impl eframe::App for Writer {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.settings);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let sc = |m, k| KeyboardShortcut::new(m, k);
        let cmd = Modifiers::COMMAND;
        let cs = cmd | Modifiers::SHIFT;
        // ⇧ 조합을 먼저 소비해야 ⌘S 가 ⇧⌘S 를 가로채지 않는다
        if ctx.input_mut(|i| i.consume_shortcut(&sc(cs, Key::S))) { self.save(true); }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(cmd, Key::S))) { self.save(false); }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(cmd, Key::O))) { self.open(); }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(cmd, Key::Comma))) { self.show_settings ^= true; }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(cs, Key::F))) { self.settings.focus ^= true; }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(cs, Key::T))) { self.settings.typewriter ^= true; }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(cs, Key::D))) { self.settings.theme = self.settings.theme.next(); }
        if self.show_settings && ctx.input(|i| i.key_pressed(Key::Escape)) { self.show_settings = false; }

        // 닫기 가드: 저장 안 된 글을 잃지 않게
        if ctx.input(|i| i.viewport().close_requested()) && self.dirty() && !self.confirm_discard() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }

        if self.loaded_font != self.settings.font {
            (self.real_italic, self.notice) = apply_fonts(&ctx, self.settings.font.as_deref());
            if self.notice.is_some() { self.settings.font = None; }
            self.loaded_font = self.settings.font.clone();
        }

        // 방해 없는 화면: 타이핑하면 숨기고, 마우스를 움직이면 보인다
        let (typed, moved) = ctx.input(|i| (
            i.events.iter().any(|e| matches!(e, egui::Event::Text(_) | egui::Event::Ime(_))),
            i.pointer.delta() != egui::Vec2::ZERO,
        ));
        if typed { self.chrome = false; }
        if moved || self.show_settings || self.notice.is_some() { self.chrome = true; }
        let alpha = ctx.animate_bool_with_time(egui::Id::new("chrome"), self.chrome, 0.25);

        let theme = self.settings.theme.resolve(&ctx);
        let p = theme.palette();
        ctx.set_theme(if theme.is_dark() { egui::Theme::Dark } else { egui::Theme::Light });
        let title = format!("{}{}", self.file_name(), if self.dirty() { " •" } else { "" });
        if ctx.input(|i| i.viewport().title.as_deref() != Some(title.as_str())) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone())); // 미션 컨트롤·Dock 용
        }

        // 타이틀바 자리: 창 끌기 + 더블클릭 확대 + 파일 이름
        egui::Panel::top("titlebar").exact_size(TITLEBAR).frame(egui::Frame::new().fill(p.bg)).show(ui, |ui| {
            let rect = ui.max_rect();
            let r = ui.interact(rect, egui::Id::new("drag"), egui::Sense::click_and_drag());
            if r.drag_started() { ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag); }
            if r.double_clicked() {
                let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
            }
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, title, FontId::proportional(13.0), p.dim.gamma_multiply(alpha));
        });

        let words = self.text.split_whitespace().count();
        let chars = self.text.chars().filter(|c| !c.is_whitespace()).count();
        egui::Panel::bottom("status").frame(egui::Frame::new().fill(p.bg).inner_margin(egui::Margin::symmetric(14, 8))).show(ui, |ui| {
            ui.set_opacity(alpha);
            ui.horizontal(|ui| {
                for (on, s) in [(self.settings.focus, "포커스"), (self.settings.typewriter, "타자기")] {
                    if on { ui.weak(s); }
                }
                if let Some(n) = &self.notice {
                    if ui.small_button(format!("⚠ {n}")).on_hover_text("클릭해서 닫기").clicked() { self.notice = None; }
                }
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if ui.add(egui::Button::new("⚙").frame(false)).on_hover_text("설정  ⌘,").clicked() {
                        self.show_settings ^= true;
                    }
                    ui.weak(format!("{words} 단어 · {chars} 자 · {}분", (words / 200).max(1)));
                });
            });
        });

        if self.show_settings {
            egui::Panel::right("settings").resizable(false).default_size(280.0)
                .frame(egui::Frame::side_top_panel(&ctx.global_style()).inner_margin(16))
                .show(ui, |ui| self.settings_panel(ui));
        }

        egui::CentralPanel::default().frame(egui::Frame::new().fill(p.bg)).show(ui, |ui| {
            let v = ui.visuals_mut();
            v.text_cursor.stroke = egui::Stroke::new(2.0, p.accent);
            v.selection.bg_fill = p.accent.gamma_multiply(if theme.is_dark() { 0.35 } else { 0.22 });
            ui.spacing_mut().scroll = egui::style::ScrollStyle::floating();

            let view_h = ui.available_height();
            let ink = Ink { p, size: self.settings.size, line: self.settings.line, real_italic: self.real_italic };
            let cw = ui.fonts_mut(|f| f.glyph_width(&body_font("body", ink.size), '0'));
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                // 본문 폭 = 글자 수 × '0' 폭. 양옆에 행잉 기호 자리(4글자)는 항상 남긴다
                let avail = ui.available_width();
                let width = (self.settings.measure as f32 * cw).min(avail - 8.0 * cw).max(cw * 10.0);
                let pad = (avail - width) / 2.0;
                let byte = self.text.char_indices().nth(self.cursor).map_or(self.text.len(), |(b, _)| b);
                let focus = self.settings.focus.then(|| para_range(&self.text, byte));
                let focus_l = focus.clone();
                let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap: f32| {
                    let font = body_font("body", ink.size);
                    let job = ui.fonts_mut(|f| layout(buf.as_str(), focus_l.clone(), ink, wrap, &mut |s| s.chars().map(|c| f.glyph_width(&font, c)).sum()));
                    ui.fonts_mut(|f| f.layout_job(job))
                };
                // 타자기 모드: 위아래 여백을 줘서 첫/마지막 줄도 화면 가운데로 올 수 있게
                let vpad = if self.settings.typewriter { view_h / 2.0 } else { 48.0 };
                ui.add_space(vpad);
                let out = ui.horizontal(|ui| {
                    ui.add_space(pad);
                    egui::TextEdit::multiline(&mut self.text)
                        .frame(egui::Frame::NONE)
                        .desired_width(width)
                        .lock_focus(true)
                        .layouter(&mut layouter)
                        .show(ui)
                }).inner;
                ui.add_space(vpad);
                paint_hanging_marks(ui, &out, &self.text, &focus, ink);
                // 다른 위젯이 포커스를 갖고 있지 않으면 항상 편집기로
                if ctx.memory(|m| m.focused().is_none()) { out.response.request_focus(); }

                if let Some(c) = out.cursor_range {
                    let moved = c.primary.index.0 != self.cursor;
                    self.cursor = c.primary.index.0;
                    if self.settings.typewriter && (moved || out.response.changed()) {
                        let r = out.galley.pos_from_cursor(c.primary).translate(out.galley_pos.to_vec2());
                        ui.scroll_to_rect(r, Some(Align::Center));
                    }
                }
            });
        });
    }
}

#[test]
fn para_range_works() {
    let t = "one\n\ntwo\nlines\n\nthree";
    assert_eq!(&t[para_range(t, 0)], "one");
    assert_eq!(&t[para_range(t, 7)], "two\nlines");
    assert_eq!(&t[para_range(t, t.len())], "three");
    assert_eq!(para_range("", 0), 0..0);
}

#[test]
fn inline_spans_works() {
    let kinds = |s: &str| inline_spans(s).into_iter().map(|(r, k)| (s[r].to_string(), k)).collect::<Vec<_>>();
    use Kind::*;
    assert_eq!(kinds("**굵게** 와 *기울임* `코드`"), vec![
        ("**".into(), Mark), ("굵게".into(), Bold), ("**".into(), Mark), (" 와 ".into(), Plain),
        ("*".into(), Mark), ("기울임".into(), Italic), ("*".into(), Mark), (" ".into(), Plain),
        ("`".into(), Mark), ("코드".into(), Code), ("`".into(), Mark),
    ]);
    assert_eq!(kinds("닫히지 않은 **별"), vec![("닫히지 않은 **별".into(), Plain)]);
    assert_eq!(kinds("snake_case_name"), vec![("snake_case_name".into(), Plain)]);
    assert_eq!(kinds("2 * 3 * 4"), vec![("2 * 3 * 4".into(), Plain)]);
    assert_eq!(kinds(""), vec![]);
}

#[test]
fn find_variants_works() {
    // macOS 기본 글꼴: Menlo.ttc 안에 Bold/Italic 면, Georgia 는 형제 파일
    let (b, i) = find_variants(Path::new(MENLO));
    assert!(b.is_some_and(|f| f.1 != 0) && i.is_some_and(|f| f.1 != 0));
    let (b, i) = find_variants(Path::new("/System/Library/Fonts/Supplemental/Georgia.ttf"));
    assert!(b.is_some_and(|f| f.0.ends_with("Georgia Bold.ttf")));
    assert!(i.is_some_and(|f| f.0.ends_with("Georgia Italic.ttf")));
    assert!(find_variants(Path::new(KOREAN)).0.is_some());
}
