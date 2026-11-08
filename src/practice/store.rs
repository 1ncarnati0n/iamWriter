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
        std::env::home_dir().unwrap_or_default().join("Library/Application Support/iamWriter")
    }
}

/// typingEng 가 쓰던 옛 파일. 테스트 빌드에서는 임시 폴더 안의 (보통 없는) 경로.
fn legacy(name: &str) -> PathBuf {
    if cfg!(test) { dir().join("legacy").join(name) } else { std::env::home_dir().unwrap_or_default().join(name) }
}

fn load<T: Default + DeserializeOwned>((name, old): (&str, &str)) -> T {
    load_first(&[dir().join(name), legacy(old)])
}

/// 앞에서부터 읽고 해석까지 되는 첫 파일. 새 파일이 없거나 깨졌을 때만 옛 파일을 읽는다.
fn load_first<T: Default + DeserializeOwned>(paths: &[PathBuf]) -> T {
    paths.iter().find_map(|p| std::fs::read(p).ok().and_then(|b| serde_json::from_slice(&b).ok())).unwrap_or_default()
}

fn save<T: Serialize>((name, _): (&str, &str), value: &T) {
    write_json(&dir().join(name), value)
}

/// 임시 파일에 쓴 뒤 rename — 쓰다가 앱이 꺼져도 기존 파일이 깨지지 않는다.
fn write_json<T: Serialize>(path: &Path, value: &T) {
    let Ok(s) = serde_json::to_string(value) else { return };
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_never_touches_real_data() {
        let home = std::env::home_dir().unwrap_or_default();
        assert!(dir().starts_with(std::env::temp_dir()), "{:?}", dir());
        assert!(!dir().starts_with(home.join("Library")));
        assert!(legacy(STATS.1).starts_with(std::env::temp_dir()), "{:?}", legacy(STATS.1));
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
        assert_eq!((s.lines_typed, s.missed["coffee"], s.en_font.as_deref()), (69, 2, Some("JetBrains Mono")));
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
