// 영어 공부 모드의 저장소: 통계·연습 설정(practice.json), 보관함(practice-library.json)
// 위치: ~/Library/Application Support/iamWriter/. 새 파일이 없을 때만 typingEng 의 옛 파일(~/.typing_eng*.json)을 읽는다.
// 옛 파일은 읽기만 하고 고치거나 지우지 않는다(백업).
use crate::practice::claude::{Engine, Line};
use crate::practice::youtube::Chunk;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// (새 파일 이름, typingEng 옛 파일 이름)
const STATS: (&str, &str) = ("practice.json", ".typing_eng.json");
const LIBRARY: (&str, &str) = ("practice-library.json", ".typing_eng_library.json");

/// 저장 폴더. 테스트 빌드에서는 프로세스별 임시 폴더라서 어떤 테스트도 실제 보관함을 덮어쓸 수 없다.
fn dir() -> PathBuf {
    if cfg!(test) {
        std::env::temp_dir().join(format!("iamwriter-practice-test-{}", std::process::id()))
    } else {
        std::env::home_dir()
            .unwrap_or_default()
            .join("Library/Application Support/iamWriter")
    }
}

/// typingEng 가 쓰던 옛 파일. 테스트 빌드에서는 임시 폴더 안의 (보통 없는) 경로.
fn legacy(name: &str) -> PathBuf {
    if cfg!(test) {
        dir().join("legacy").join(name)
    } else {
        std::env::home_dir().unwrap_or_default().join(name)
    }
}

fn load<T: Default + DeserializeOwned>((name, old): (&str, &str)) -> T {
    load_first(&[dir().join(name), legacy(old)])
}

/// 앞에서부터 읽고 해석까지 되는 첫 파일. 새 파일이 없거나 깨졌을 때만 옛 파일을 읽는다.
fn load_first<T: Default + DeserializeOwned>(paths: &[PathBuf]) -> T {
    paths
        .iter()
        .find_map(|p| {
            std::fs::read(p)
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
        })
        .unwrap_or_default()
}

fn save<T: Serialize>((name, _): (&str, &str), value: &T) {
    write_json(&dir().join(name), value)
}

/// 임시 파일에 쓴 뒤 rename — 쓰다가 앱이 꺼져도 기존 파일이 깨지지 않는다.
fn write_json<T: Serialize>(path: &Path, value: &T) {
    let Ok(s) = serde_json::to_string(value) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, s).is_ok() {
        let _ = std::fs::rename(tmp, path);
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct Stats {
    pub lines_typed: u32,
    pub best_wpm: f64,
    pub missed: HashMap<String, u32>,
    /// 설정: 유튜브 자막을 받을 때 Chrome 로그인 쿠키를 쓰지 않음 (기본은 사용)
    #[serde(default)]
    pub no_yt_cookies: bool,
    /// 설정: 따라치기(영문) 글꼴, 뜻(한글) 글꼴. None = 기본값
    #[serde(default)]
    pub en_font: Option<String>,
    #[serde(default)]
    pub ko_font: Option<String>,
    /// 설정: 문장 생성·정리·첨삭에 쓸 CLI
    #[serde(default)]
    pub engine: Engine,
    /// 첨삭 오류 유형별 누적 횟수 (Kind::key → 횟수). 처음 쓴 글만 센다.
    #[serde(default)]
    pub error_kinds: HashMap<String, u32>,
}

impl Stats {
    pub fn load() -> Self {
        load(STATS)
    }
    pub fn save(&self) {
        save(STATS, self)
    }
}

/// Claude로 만든 문장 한 묶음.
#[derive(Clone, Serialize, Deserialize)]
pub struct Lesson {
    pub title: String,
    pub lines: Vec<Line>,
}

/// 유튜브 영상: 자막 세트 전체 + Claude로 변환을 마친 세트 + 진행 위치.
#[derive(Clone, Serialize, Deserialize)]
pub struct Video {
    pub id: String,
    pub title: String,
    pub chunks: Vec<Chunk>,
    /// chunks와 같은 길이. 변환 전이면 None.
    pub sets: Vec<Option<Vec<Line>>>,
    /// 다음에 할 세트 번호
    pub progress: usize,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Library {
    pub lessons: Vec<Lesson>,
    pub videos: Vec<Video>,
    /// 시간 두고 다시 영작할 문장들. 보관함에서 지워도 남도록 문장을 통째로 복사해 둔다.
    #[serde(default)]
    pub deck: Vec<Card>,
}

/// 복습 카드: 문장 + 다음 복습 날짜.
#[derive(Clone, Serialize, Deserialize)]
pub struct Card {
    pub line: Line,
    /// 다음 복습 날 (today() 기준 날짜 번호)
    pub due: u32,
    /// 지난번에 정한 간격(일)
    pub interval: u32,
}

/// 오늘 날짜 번호(1970-01-01부터 며칠째).
// ponytail: UTC 기준이라 한국에선 오전 9시에 날짜가 바뀐다. 자정이 중요해지면 로컬 시간대 오프셋을 더한다.
pub fn today() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| (d.as_secs() / 86_400) as u32)
}

/// 라이트너 상자의 간격(일). 맞히면 한 상자 위로, 틀리면 한 상자 아래로.
const BOXES: [u32; 5] = [1, 3, 7, 14, 30];

/// 복습 결과로 다음 간격(일)을 정한다. prev 는 지난 간격(새 카드는 0).
/// 새 카드는 따라치기로 이미 한 번 봤으니 첫 상자(1일)에 있는 것으로 친다 → 맞히면 3일.
/// 마지막 상자(30일)에서 또 맞히면 30일에 머문다.
/// 틀리면 한 상자만 내려간다(첫 상자로 되돌리면 한 번 실수에 쌓은 간격이 다 사라져서).
pub fn next_interval(prev: u32, passed: bool) -> u32 {
    // 지금 상자: prev 이하인 가장 큰 상자 (상자 사이 값은 옛 규칙으로 저장된 간격)
    let cur = BOXES.iter().rposition(|&d| d <= prev).unwrap_or(0);
    let next = if passed {
        (cur + 1).min(BOXES.len() - 1)
    } else {
        cur.saturating_sub(1)
    };
    BOXES[next]
}

impl Library {
    pub fn load() -> Self {
        load(LIBRARY)
    }
    pub fn save(&self) {
        save(LIBRARY, self)
    }
    pub fn video(&self, id: &str) -> Option<&Video> {
        self.videos.iter().find(|v| v.id == id)
    }
    pub fn video_mut(&mut self, id: &str) -> Option<&mut Video> {
        self.videos.iter_mut().find(|v| v.id == id)
    }

    /// 따라친 문장을 복습 카드로. 이미 있는 문장(영어가 같은 것)은 건너뛴다. 첫 복습은 내일.
    pub fn add_to_deck(&mut self, lines: &[Line], today: u32) {
        for l in lines {
            if !self.deck.iter().any(|c| c.line.en == l.en) {
                self.deck.push(Card {
                    line: l.clone(),
                    due: today + 1,
                    interval: 0,
                });
            }
        }
    }

    /// 오늘 복습할 카드 번호(오래 밀린 것부터), 최대 max 개.
    pub fn due(&self, today: u32, max: usize) -> Vec<usize> {
        let mut v: Vec<usize> = (0..self.deck.len())
            .filter(|&i| self.deck[i].due <= today)
            .collect();
        v.sort_by_key(|&i| self.deck[i].due);
        v.truncate(max);
        v
    }

    pub fn grade(&mut self, card: usize, passed: bool, today: u32) {
        if let Some(c) = self.deck.get_mut(card) {
            c.interval = next_interval(c.interval, passed);
            c.due = today + c.interval;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_never_touches_real_data() {
        let home = std::env::home_dir().unwrap_or_default();
        assert!(dir().starts_with(std::env::temp_dir()), "{:?}", dir());
        assert!(!dir().starts_with(home.join("Library")));
        assert!(
            legacy(STATS.1).starts_with(std::env::temp_dir()),
            "{:?}",
            legacy(STATS.1)
        );
    }

    #[test]
    fn old_files_without_new_fields_still_load() {
        // 새 필드(error_kinds, deck)가 없던 시절의 파일. 읽기에 실패하면 빈 데이터로 시작해 다음 저장 때 덮어쓴다.
        let s: Stats =
            serde_json::from_str(r#"{"lines_typed":3,"best_wpm":40.0,"missed":{}}"#).unwrap();
        assert!(s.lines_typed == 3 && s.error_kinds.is_empty());
        let l: Library = serde_json::from_str(
            r#"{"lessons":[{"title":"t","lines":[{"speaker":"","en":"Hi.","ko":"안녕."}]}],"videos":[]}"#,
        )
        .unwrap();
        assert!(l.lessons.len() == 1 && l.deck.is_empty());
    }

    #[test]
    fn deck_schedules_and_dedupes() {
        let line = |en: &str| Line {
            speaker: String::new(),
            en: en.into(),
            ko: "뜻".into(),
            start: 0.0,
            end: 0.0,
        };
        let mut lib = Library::default();
        lib.add_to_deck(&[line("A."), line("B."), line("A.")], 100);
        assert_eq!(lib.deck.len(), 2, "같은 문장은 한 장");
        assert!(lib.due(100, 10).is_empty(), "오늘 넣은 건 내일부터");
        assert_eq!(lib.due(101, 10), vec![0, 1]);
        assert_eq!(lib.due(101, 1).len(), 1, "한 번에 max 개까지");
        lib.grade(0, true, 101);
        lib.grade(1, false, 101);
        assert!(
            lib.deck[0].due > lib.deck[1].due,
            "맞힌 카드가 더 늦게 돌아온다"
        );
        assert!(lib.deck[1].due > 101, "틀려도 오늘 다시 나오진 않는다");
    }

    #[test]
    fn interval_grows_on_pass_and_shrinks_on_fail() {
        let mut d = 0;
        let mut seen = vec![];
        for _ in 0..4 {
            d = next_interval(d, true);
            seen.push(d);
        }
        assert!(seen.windows(2).all(|w| w[1] > w[0]), "{seen:?}");
        assert!(next_interval(d, false) < d);
        assert!(next_interval(0, false) >= 1);
    }

    #[test]
    fn leitner_boxes() {
        let walk: Vec<u32> = (0..6)
            .scan(0, |d, _| {
                *d = next_interval(*d, true);
                Some(*d)
            })
            .collect();
        assert_eq!(
            walk,
            [3, 7, 14, 30, 30, 30],
            "맞히면 한 상자씩, 30일에서 머묾"
        );
        assert_eq!(next_interval(30, false), 14, "틀리면 한 상자 아래로");
        assert_eq!(next_interval(3, false), 1);
        assert_eq!(next_interval(1, false), 1, "첫 상자 아래로는 안 내려감");
        assert_eq!(next_interval(0, false), 1);
        // 옛 임시 규칙으로 저장된 간격(예: 4일)도 다음 상자로 이어진다
        assert_eq!(next_interval(4, true), 7);
    }

    #[test]
    fn reads_legacy_only_when_new_file_is_missing_and_never_touches_it() {
        let d = std::env::temp_dir().join(format!("iamwriter-migrate-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let (new, old) = (d.join("app/practice.json"), d.join(".typing_eng.json"));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(&old, r#"{"lines_typed":69,"best_wpm":33.0,"missed":{"coffee":2},"en_font":"JetBrains Mono"}"#).unwrap();
        let before = std::fs::read(&old).unwrap();

        // 새 파일이 없으면 옛 파일을 읽는다
        let mut s: Stats = load_first(&[new.clone(), old.clone()]);
        assert_eq!(
            (s.lines_typed, s.missed["coffee"], s.en_font.as_deref()),
            (69, 2, Some("JetBrains Mono"))
        );
        // 저장은 새 위치에만 (폴더가 없어도 만든다)
        s.lines_typed = 70;
        write_json(&new, &s);
        let s: Stats = load_first(&[new.clone(), old.clone()]);
        assert_eq!(s.lines_typed, 70);
        assert_eq!(std::fs::read(&old).unwrap(), before, "옛 파일은 그대로");
        // 새 파일이 깨졌으면 옛 파일로
        std::fs::write(&new, "{").unwrap();
        assert_eq!(load_first::<Stats>(&[new, old]).lines_typed, 69);
        std::fs::remove_dir_all(d).unwrap();
    }
}
