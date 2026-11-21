// AI 엔진 세 가지: 로그인된 Claude Code CLI(`claude -p`), Claude API(HTTP, API 키), Codex CLI(`codex exec`).
// 동기(blocking) 호출이므로 UI 스레드 밖에서 부른다.
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Mutex;

/// 문장 생성·정리·첨삭을 맡길 엔진. 저장 파일에는 이 이름(Claude/Codex/Api)으로 들어간다.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Engine {
    #[default]
    Claude,
    Codex,
    Api,
}

/// Claude 모델 (ID, 라벨) — API 키가 없거나 목록을 아직 못 받았을 때 쓰는 고정 목록.
/// 키가 있으면 GET /v1/models 의 실시간 목록(refresh_models)이 대신한다.
/// 이 앱은 JSON 스키마 구조화 출력(output_config.format)이 꼭 필요해서 그것을 지원하는 모델만 둔다.
const CLAUDE_MODELS: &[(&str, &str)] = &[
    ("claude-haiku-5-5", "Haiku 5.5"),
    ("claude-sonnet-5-5", "Sonnet 5.5"),
    ("claude-opus-5-5", "Opus 5.5"),
    ("claude-fable-5-2", "Fable 5.2"),
    ("claude-fable-5-1", "Fable 5.1"),
    ("claude-fable-5", "Fable 5"),
    ("claude-opus-5", "Opus 5"),
    ("claude-opus-4-8", "Opus 4.8"),
    ("claude-sonnet-5", "Sonnet 5"),
    ("claude-haiku-4-5", "Haiku 4.5"),
];
/// Claude Code 의 `haiku` 별칭: CLI 가 늘 최신 Haiku 로 푼다(예전 기본값).
const CLAUDE_CODE_DEFAULT: &str = "haiku";
const API_DEFAULT: &str = "claude-haiku-5-5";
const API_URL: &str = "https://api.anthropic.com/v1/messages";
const MODELS_URL: &str = "https://api.anthropic.com/v1/models";
const API_VERSION: &str = "2026-10-21";
/// 서버 쪽 대체 모델(거절되면 다른 모델로 다시)을 받는 모델. Haiku 는 받지 않는다.
const FALLBACK_MODELS: &[&str] = &[
    "claude-fable-5-2",
    "claude-fable-5-1",
    "claude-opus-5-5",
    "claude-opus-5",
    "claude-sonnet-5-5",
];
const KEYCHAIN_SERVICE: &str = "iamWriter Anthropic API";
const KEYCHAIN_ACCOUNT: &str = "iamWriter";

/// Models API 에서 받은 모델 하나
#[derive(Clone, Debug, PartialEq)]
pub struct LiveModel {
    pub id: String,
    pub label: String,
    /// output_config.effort 를 받는지 (Haiku 4.5 는 받지 않는다)
    pub effort: bool,
}

/// 실시간 모델 목록. 비었으면 아직 못 받은 것 → CLAUDE_MODELS.
static LIVE_MODELS: Mutex<Vec<LiveModel>> = Mutex::new(Vec::new());

/// Claude 모델 (ID, 라벨): 실시간 목록이 있으면 그것, 없으면 고정 목록.
fn claude_models() -> Vec<(String, String)> {
    let live = LIVE_MODELS.lock().unwrap();
    if live.is_empty() {
        CLAUDE_MODELS
            .iter()
            .map(|&(id, l)| (id.to_string(), l.to_string()))
            .collect()
    } else {
        live.iter()
            .map(|m| (m.id.clone(), m.label.clone()))
            .collect()
    }
}

fn live_loaded() -> bool {
    !LIVE_MODELS.lock().unwrap().is_empty()
}

/// Claude Code 를 `haiku` 별칭으로 부르면 실제로 풀린 모델 ID를 응답에서 읽어 둔다.
static CLAUDE_MODEL: Mutex<Option<String>> = Mutex::new(None);
const CODEX_FALLBACK: &str = "gpt-6.1-terra";

impl Engine {
    pub const ALL: [Engine; 3] = [Engine::Claude, Engine::Api, Engine::Codex];

    pub fn name(self) -> &'static str {
        match self {
            Engine::Claude => "Claude Code",
            Engine::Api => "Claude API",
            Engine::Codex => "Codex",
        }
    }

    /// 어떻게 연결하는지 한 줄
    pub fn how(self) -> &'static str {
        match self {
            Engine::Claude => {
                "로그인된 Claude Code CLI(`claude`)를 씁니다. API 키가 필요 없습니다. 터미널에서 `claude`로 한 번 로그인하세요."
            }
            Engine::Api => {
                "Anthropic API를 직접 부릅니다. API 키가 필요하고 쓴 만큼 과금됩니다. 키는 macOS 키체인에 저장합니다(ANTHROPIC_API_KEY 환경변수가 있으면 그것을 먼저 씁니다)."
            }
            Engine::Codex => {
                "로그인된 Codex CLI(`codex`)를 씁니다. API 키가 필요 없습니다. 터미널에서 `codex login`으로 로그인하세요."
            }
        }
    }

    /// 고를 수 있는 모델 (ID, 라벨). 첫 번째가 기본값.
    pub fn models(self) -> Vec<(String, String)> {
        let claude = claude_models().into_iter();
        match self {
            Engine::Claude => std::iter::once((CLAUDE_CODE_DEFAULT.into(), "Haiku (최신)".into()))
                .chain(claude)
                .collect(),
            Engine::Api => claude.collect(),
            Engine::Codex => codex_models()
                .iter()
                .map(|m| (m.clone(), m.clone()))
                .collect(),
        }
    }

    fn default_model(self) -> String {
        match self {
            Engine::Claude => CLAUDE_CODE_DEFAULT.into(),
            Engine::Api => API_DEFAULT.into(),
            Engine::Codex => latest_terra(codex_models()).unwrap_or_else(|| CODEX_FALLBACK.into()),
        }
    }

    /// 저장된 모델이 지금 목록에 있으면 그것, 없으면 기본값.
    /// Claude 모델은 실시간 목록을 받기 전이면 저장된 이름을 믿는다(고정 목록에 없는 새 모델일 수 있다).
    pub fn with_model(self, saved: Option<&String>) -> Ai {
        let trust = self != Engine::Codex && !live_loaded();
        let model = saved
            .filter(|m| !m.is_empty())
            .filter(|m| trust || self.models().iter().any(|(id, _)| id == *m))
            .cloned()
            .unwrap_or_else(|| self.default_model());
        Ai {
            engine: self,
            model,
        }
    }
}

/// 엔진 + 모델: 호출 한 번에 필요한 전부.
#[derive(Clone, Debug, PartialEq)]
pub struct Ai {
    pub engine: Engine,
    pub model: String,
}

impl Ai {
    /// 화면에 보여줄 "엔진 · 모델".
    pub fn label(&self) -> String {
        let model = match (self.engine, self.model.as_str()) {
            (Engine::Claude, CLAUDE_CODE_DEFAULT) => CLAUDE_MODEL
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| "Haiku (최신)".into()),
            (Engine::Codex, m) => m.to_string(),
            (_, m) => claude_models()
                .into_iter()
                .find(|(id, _)| id == m)
                .map_or_else(|| m.to_string(), |(_, l)| l),
        };
        format!("{} · {model}", self.engine.name())
    }
}

/// Codex가 받아 둔 모델 목록(~/.codex/models_cache.json)의 공개 모델, 최신 순. 없으면 고정값 하나.
fn codex_models() -> &'static [String] {
    // 화면을 그릴 때마다 부르므로 실행 중 한 번만 읽는다.
    static MODELS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    MODELS.get_or_init(|| {
        let path = std::env::home_dir()
            .unwrap_or_default()
            .join(".codex/models_cache.json");
        let v: Value = std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let list = listed_models(&v);
        if list.is_empty() {
            vec![CODEX_FALLBACK.into()]
        } else {
            list
        }
    })
}

/// visibility 가 list 인 모델, priority 가 작을수록(최신) 앞.
fn listed_models(v: &Value) -> Vec<String> {
    let mut list: Vec<_> = v["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| m["visibility"] == "list")
        .filter_map(|m| {
            Some((
                m["priority"].as_i64().unwrap_or(i64::MAX),
                m["slug"].as_str()?.to_string(),
            ))
        })
        .collect();
    list.sort();
    list.into_iter().map(|(_, s)| s).collect()
}

/// 예전 기본값: 가장 최신 Terra.
fn latest_terra(models: &[String]) -> Option<String> {
    models.iter().find(|m| m.contains("terra")).cloned()
}

// ---- API 키 (macOS 키체인) ----

/// API 키가 어디서 오는지
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum KeySource {
    Env,
    Keychain,
    Missing,
}

fn env_key() -> Option<String> {
    std::env::var("ANTHROPIC_API_KEY")
        .ok()
        .filter(|k| !k.trim().is_empty())
}

fn keychain_key() -> Option<String> {
    let out = Command::new("security")
        .args([
            "find-generic-password",
            "-a",
            KEYCHAIN_ACCOUNT,
            "-s",
            KEYCHAIN_SERVICE,
            "-w",
        ])
        .output()
        .ok()?;
    let key = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (out.status.success() && !key.is_empty()).then_some(key)
}

pub fn key_source() -> KeySource {
    if env_key().is_some() {
        KeySource::Env
    } else if keychain_key().is_some() {
        KeySource::Keychain
    } else {
        KeySource::Missing
    }
}

/// 키는 영문·숫자·`-`·`_` 만. `security` 명령 줄에 그대로 들어가므로 그 밖의 글자는 막는다.
fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// 키체인에 저장(있으면 덮어씀). 키가 명령 인수(ps 에 보임)에 남지 않게 `security -i` 의 입력으로 넘긴다.
pub fn save_key(key: &str) -> Result<(), String> {
    let key = key.trim();
    if !valid_key(key) {
        return Err("API 키 형식이 아닙니다 (sk-ant-… 로 시작하는 키를 붙여 넣으세요).".into());
    }
    let mut child = Command::new("security")
        .arg("-i")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("키체인 실행 실패: {e}"))?;
    let line = format!(
        "add-generic-password -U -a \"{KEYCHAIN_ACCOUNT}\" -s \"{KEYCHAIN_SERVICE}\" -w \"{key}\"\n"
    );
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(line.as_bytes())
        .map_err(|e| format!("키체인 쓰기 실패: {e}"))?;
    let out = child
        .wait_with_output()
        .map_err(|e| format!("키체인 실패: {e}"))?;
    let err = String::from_utf8_lossy(&out.stderr);
    if !out.status.success() || !err.trim().is_empty() {
        return Err(format!("키체인 저장 실패: {}", err.trim()));
    }
    Ok(())
}

pub fn delete_key() {
    let _ = Command::new("security")
        .args([
            "delete-generic-password",
            "-a",
            KEYCHAIN_ACCOUNT,
            "-s",
            KEYCHAIN_SERVICE,
        ])
        .output();
}

const SYSTEM: &str = "You are a friendly English tutor for Korean learners. Answer only through the structured output.";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Line {
    /// 대화형일 때 화자 이름(타이핑 대상 아님). 소설이면 빈 문자열.
    #[serde(default)]
    pub speaker: String,
    pub en: String,
    pub ko: String,
    /// 유튜브 세트일 때 영상 구간(초). Claude 응답에는 없고 youtube::align이 채운다.
    #[serde(default)]
    pub start: f64,
    #[serde(default)]
    pub end: f64,
}

impl Line {
    /// "Mia: " 형태의 화자 라벨. 소설이면 빈 문자열.
    pub fn who(&self) -> String {
        if self.speaker.is_empty() {
            String::new()
        } else {
            format!("{}: ", self.speaker)
        }
    }
}

#[derive(Deserialize)]
struct Lines {
    lines: Vec<Line>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Review {
    pub corrected: String,
    pub feedback_ko: String,
    pub errors: Vec<Fix>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Fix {
    pub original: String,
    pub fix: String,
    pub why_ko: String,
    #[serde(default)]
    pub kind: Kind,
    /// 몇 번째 한국어 줄의 실수인지(1부터). 모르면 0.
    #[serde(default)]
    pub line: usize,
}

/// 첨삭 오류 유형. 스키마의 enum 과 같은 이름(snake_case). 모르는 값은 Other.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Article,
    Tense,
    Preposition,
    Agreement,
    WordChoice,
    WordOrder,
    Spelling,
    Punctuation,
    Missing,
    #[default]
    #[serde(other)]
    Other,
}

impl Kind {
    pub const ALL: [Kind; 10] = [
        Kind::Article,
        Kind::Tense,
        Kind::Preposition,
        Kind::Agreement,
        Kind::WordChoice,
        Kind::WordOrder,
        Kind::Spelling,
        Kind::Punctuation,
        Kind::Missing,
        Kind::Other,
    ];

    /// 스키마·저장 파일에 쓰는 이름
    pub fn key(self) -> &'static str {
        match self {
            Kind::Article => "article",
            Kind::Tense => "tense",
            Kind::Preposition => "preposition",
            Kind::Agreement => "agreement",
            Kind::WordChoice => "word_choice",
            Kind::WordOrder => "word_order",
            Kind::Spelling => "spelling",
            Kind::Punctuation => "punctuation",
            Kind::Missing => "missing",
            Kind::Other => "other",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Article => "관사",
            Kind::Tense => "시제",
            Kind::Preposition => "전치사",
            Kind::Agreement => "수 일치",
            Kind::WordChoice => "어휘",
            Kind::WordOrder => "어순",
            Kind::Spelling => "철자",
            Kind::Punctuation => "구두점",
            Kind::Missing => "빠뜨림",
            Kind::Other => "기타",
        }
    }

    pub fn from_key(k: &str) -> Kind {
        Kind::ALL
            .into_iter()
            .find(|x| x.key() == k)
            .unwrap_or_default()
    }
}

/// AI 문장의 장르
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Genre {
    Chat,
    Story,
    /// Claude Code 같은 AI 코딩 에이전트에게 개발 중에 보내는 영어 프롬프트
    Prompt,
}

impl Genre {
    pub const ALL: [Genre; 3] = [Genre::Chat, Genre::Story, Genre::Prompt];

    pub fn label(self) -> &'static str {
        match self {
            Genre::Chat => "채팅",
            Genre::Story => "소설",
            Genre::Prompt => "Claude Code 프롬프트",
        }
    }

    /// 주제 예시 (누르면 주제 칸에 들어간다)
    pub fn examples(self) -> &'static [&'static str] {
        match self {
            Genre::Chat => &[
                "ordering coffee",
                "weekend plans",
                "a job interview tomorrow",
                "recommending a TV show",
                "planning a trip to Jeju",
                "a late-night coding session",
            ],
            Genre::Story => &[
                "a rainy day",
                "a lost cat",
                "the first day at a new job",
                "a robot that learns to cook",
                "a missed train",
                "a bug that only happens on Fridays",
            ],
            Genre::Prompt => &[
                "fix a failing unit test",
                "add a dark mode toggle",
                "refactor a long function",
                "explain how this module works",
                "debug a crash on startup",
                "write a commit message and open a pull request",
                "add an API endpoint with validation",
                "review my changes for bugs",
                "speed up a slow database query",
                "write a README for the project",
            ],
        }
    }

    /// 생성 프롬프트의 장르 설명
    fn kind(self) -> &'static str {
        match self {
            Genre::Chat => {
                "a casual chat/messenger conversation between two friends. Put the speaker's first name in `speaker` and only the spoken words in `en` (never include the name or a colon prefix in `en`)"
            }
            Genre::Story => {
                "a short, simple story told in past tense. Leave `speaker` as an empty string"
            }
            Genre::Prompt => {
                "a realistic sequence of prompts a software developer types to Claude Code (an AI coding agent in the terminal) while working on one task, from describing the goal to asking for tests and a commit. \
                 Each line is one complete instruction, question or follow-up to the agent, written the way developers actually write prompts: \
                 clear, specific, polite but direct, naming files, functions, errors or constraints when useful (keep names short, no backticks or quotes). \
                 Leave `speaker` as an empty string"
            }
        }
    }
}

pub fn generate_lines(
    ai: &Ai,
    level: &str,
    genre: Genre,
    topic: &str,
) -> Result<Vec<Line>, String> {
    let kind = genre.kind();
    let prompt = format!(
        "Write {kind} about \"{topic}\" for a Korean learner of English at CEFR level {level}.\n\
         8 to 12 lines, one short sentence per line, natural everyday English, plain ASCII punctuation only (no curly quotes, no em dashes).\n\
         For each line give `speaker`, `en` (the English) and `ko` (a natural Korean translation of `en` only)."
    );
    call_lines(ai, &prompt)
}

/// 유튜브 자막 조각 → 문장 단위 Line (원문 단어 유지, 구두점 복원, 한국어 번역).
pub fn lines_from_transcript(ai: &Ai, chunk: &str) -> Result<Vec<Line>, String> {
    let prompt = format!(
        "Below is a raw fragment of an English YouTube transcript (it may lack punctuation and start or end mid-sentence).\n\n\
         {chunk}\n\n\
         Turn it into typing practice lines for a Korean learner:\n\
         - Keep the speaker's words in the same order. Only restore punctuation and capitalization; you may drop filler like \"um\", \"uh\". Do not reorder, summarize or add words.\n\
         - One sentence per line; split very long sentences at a comma or conjunction.\n\
         - Plain ASCII punctuation only (no curly quotes, no em dashes, no music notes).\n\
         - `speaker` is an empty string. `ko` is a natural Korean translation of `en`."
    );
    call_lines(ai, &prompt)
}

fn call_lines(ai: &Ai, prompt: &str) -> Result<Vec<Line>, String> {
    let schema = json!({
        "type": "object",
        "properties": {
            "lines": {"type": "array", "items": {
                "type": "object",
                "properties": {
                    "speaker": {"type": "string"},
                    "en": {"type": "string"},
                    "ko": {"type": "string"}
                },
                "required": ["speaker", "en", "ko"],
                "additionalProperties": false
            }}
        },
        "required": ["lines"],
        "additionalProperties": false
    });
    let out: Lines = call(ai, prompt, schema)?;
    if out.lines.is_empty() {
        return Err("빈 결과가 왔습니다. 다시 시도하세요.".into());
    }
    Ok(out.lines)
}

pub fn review_writing(ai: &Ai, lines: &[Line], written: &str) -> Result<Review, String> {
    let ko: Vec<String> = lines
        .iter()
        .map(|l| format!("{}{}", l.who(), l.ko))
        .collect();
    let model: Vec<String> = lines
        .iter()
        .map(|l| format!("{}{}", l.who(), l.en))
        .collect();
    // 줄 번호를 붙여 보내야 오류마다 몇 번째 줄인지 받을 수 있다(복습 일정에 쓴다)
    let numbered = |v: Vec<String>| {
        v.iter()
            .enumerate()
            .map(|(i, s)| format!("{}. {s}", i + 1))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let prompt = format!(
        "A Korean learner translated these Korean lines into English.\n\n\
         Korean:\n{}\n\nReference English:\n{}\n\nLearner's English:\n{}\n\n\
         Review the learner's English. Do not require it to match the reference; accept any natural, correct English. The learner may omit speaker names; that is fine.\n\
         - `corrected`: the learner's text with minimal fixes so it is correct and natural.\n\
         - `errors`: each mistake as original fragment, fixed fragment, and a short explanation in Korean.\n\
           `kind` is the error type. `line` is the number of the Korean line the mistake belongs to.\n\
           If a Korean line was not translated at all, add one error for it with an empty `original`, the reference sentence as `fix`, and kind `missing`.\n\
         - `feedback_ko`: 2-3 encouraging sentences in Korean: what they did well and the one error type to focus on next. \
           If the lines are prompts to an AI coding assistant, also say in one sentence whether the learner's prompts are clear and specific enough for the assistant to act on.",
        numbered(ko),
        numbered(model),
        written
    );
    let schema = json!({
        "type": "object",
        "properties": {
            "corrected": {"type": "string"},
            "feedback_ko": {"type": "string"},
            "errors": {"type": "array", "items": {
                "type": "object",
                "properties": {
                    "original": {"type": "string"},
                    "fix": {"type": "string"},
                    "why_ko": {"type": "string"},
                    "kind": {"type": "string", "enum": Kind::ALL.map(Kind::key)},
                    "line": {"type": "integer"}
                },
                "required": ["original", "fix", "why_ko", "kind", "line"],
                "additionalProperties": false
            }}
        },
        "required": ["corrected", "feedback_ko", "errors"],
        "additionalProperties": false
    });
    call(ai, &prompt, schema)
}

fn call<T: DeserializeOwned>(ai: &Ai, prompt: &str, schema: Value) -> Result<T, String> {
    match ai.engine {
        Engine::Claude => call_claude(&ai.model, prompt, schema),
        Engine::Api => call_api(&ai.model, prompt, schema),
        Engine::Codex => call_codex(&ai.model, prompt, schema),
    }
}

/// 연결 확인: 아주 짧은 구조화 호출 한 번. 성공하면 "엔진 · 모델".
pub fn check(ai: &Ai) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Ok_ {
        #[allow(dead_code)]
        ok: String,
    }
    let schema = json!({
        "type": "object",
        "properties": {"ok": {"type": "string"}},
        "required": ["ok"],
        "additionalProperties": false
    });
    let _: Ok_ = call(ai, "Reply with ok set to \"yes\".", schema)?;
    Ok(ai.label())
}

/// Claude API(Messages).
fn call_api<T: DeserializeOwned>(model: &str, prompt: &str, schema: Value) -> Result<T, String> {
    let mut body = json!({
        "model": model,
        "max_tokens": 16000,
        "system": SYSTEM,
        "messages": [{"role": "user", "content": prompt}],
        "output_config": {"format": {"type": "json_schema", "schema": schema}}
    });
    // 짧은 연습 문장이라 깊게 생각할 필요가 없다. effort 를 받지 않는 모델(Haiku 4.5)엔 넣지 않는다.
    let effort = LIVE_MODELS
        .lock()
        .unwrap()
        .iter()
        .find(|m| m.id == model)
        .map_or(model != "claude-haiku-4-5", |m| m.effort);
    if effort {
        body["output_config"]["effort"] = json!("low");
    }
    let fallback = FALLBACK_MODELS.contains(&model);
    let mut headers = vec!["content-type: application/json"];
    if fallback {
        body["fallbacks"] = json!("default");
        headers.push("anthropic-beta: server-side-fallback-2026-07-01");
    }
    api_result(&api_request(API_URL, &headers, Some(body.to_string()))?)
}

/// API 키로 Claude API 를 부르고 JSON 을 돌려준다. body 가 없으면 GET.
/// 키는 curl 설정으로 표준 입력에 넘겨 명령 인수(ps 에 보임)에 남지 않게 한다.
fn api_request(url: &str, headers: &[&str], body: Option<String>) -> Result<Value, String> {
    let key = env_key().or_else(keychain_key).ok_or(
        "Claude API 키가 없습니다. 설정 → 영어 공부에서 키를 저장하거나 ANTHROPIC_API_KEY 를 설정하세요.",
    )?;
    if !valid_key(&key) {
        return Err("API 키 형식이 올바르지 않습니다. 설정에서 다시 저장하세요.".into());
    }
    let mut cmd = Command::new("curl");
    cmd.args(["-sS", "--max-time", "300", "-K", "-", url])
        .args(["-H", &format!("anthropic-version: {API_VERSION}")]);
    for h in headers {
        cmd.args(["-H", h]);
    }
    if let Some(b) = body {
        cmd.arg("--data-binary").arg(b);
    }
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("curl 실행 실패: {e}"))?;
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(format!("header = \"x-api-key: {key}\"\n").as_bytes())
        .map_err(|e| format!("curl 입력 실패: {e}"))?;
    let out = child
        .wait_with_output()
        .map_err(|e| format!("curl 실패: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "Claude API 연결 실패: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let v: Value = serde_json::from_slice(&out.stdout)
        .map_err(|_| "Claude API 응답을 읽을 수 없습니다.".to_string())?;
    api_error(&v).map_or(Ok(v), Err)
}

/// 오류 응답이면 한국어 안내.
fn api_error(v: &Value) -> Option<String> {
    if v["type"] != "error" {
        return None;
    }
    let msg = v["error"]["message"].as_str().unwrap_or("알 수 없는 오류");
    Some(match v["error"]["type"].as_str() {
        Some("authentication_error") => format!("Claude API 키가 거부됐습니다: {msg}"),
        Some("rate_limit_error") => {
            format!("Claude API 요청 한도에 걸렸습니다. 잠시 뒤 다시: {msg}")
        }
        _ => format!("Claude API 오류: {msg}"),
    })
}

/// GET /v1/models 로 실시간 모델 목록을 받아 둔다(페이지를 끝까지). 받은 개수를 돌려준다.
pub fn refresh_models() -> Result<usize, String> {
    let mut all = vec![];
    let mut after: Option<String> = None;
    loop {
        let url = match &after {
            Some(id) => format!("{MODELS_URL}?limit=1000&after_id={id}"),
            None => format!("{MODELS_URL}?limit=1000"),
        };
        let page = api_request(&url, &[], None)?;
        all.extend(live_models(&page));
        match page["last_id"].as_str() {
            Some(id) if page["has_more"] == true => after = Some(id.to_string()),
            _ => break,
        }
    }
    if all.is_empty() {
        return Err("구조화 출력을 지원하는 모델이 목록에 없습니다.".into());
    }
    let n = all.len();
    *LIVE_MODELS.lock().unwrap() = all;
    Ok(n)
}

/// 모델 목록 한 페이지 → 구조화 출력을 지원하는 모델만 (API 가 준 순서 = 최신 순).
fn live_models(page: &Value) -> Vec<LiveModel> {
    page["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| m["capabilities"]["structured_outputs"]["supported"] == true)
        .filter_map(|m| {
            let id = m["id"].as_str()?.to_string();
            let name = m["display_name"].as_str().unwrap_or(&id);
            Some(LiveModel {
                label: name.strip_prefix("Claude ").unwrap_or(name).to_string(),
                effort: m["capabilities"]["effort"]["low"]["supported"] == true,
                id,
            })
        })
        .collect()
}

/// Messages 응답 → 구조화 결과. 오류 · 거절 · 길이 초과는 한국어 안내로.
fn api_result<T: DeserializeOwned>(v: &Value) -> Result<T, String> {
    if let Some(e) = api_error(v) {
        return Err(e);
    }
    match v["stop_reason"].as_str() {
        Some("refusal") => {
            return Err("모델이 이 요청을 거절했습니다. 주제나 문장을 바꿔 보세요.".into());
        }
        Some("max_tokens") => return Err("응답이 너무 길어 잘렸습니다. 다시 시도하세요.".into()),
        _ => {}
    }
    // thinking 블록은 건너뛰고 text 블록만 (구조화 출력은 text 로 온다)
    let text: String = v["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|b| b["type"] == "text")
        .filter_map(|b| b["text"].as_str())
        .collect();
    serde_json::from_str(&text).map_err(|e| format!("구조화 응답 형식 오류: {e}"))
}

fn call_codex<T: DeserializeOwned>(model: &str, prompt: &str, schema: Value) -> Result<T, String> {
    // 스키마와 답을 파일로 주고받는다. 호출마다 다른 이름.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let base =
        std::env::temp_dir().join(format!("typing_eng_codex_{}_{nanos}", std::process::id()));
    let (schema_path, out_path) = (
        base.with_extension("schema.json"),
        base.with_extension("out.json"),
    );
    std::fs::write(&schema_path, schema.to_string())
        .map_err(|e| format!("임시 파일 쓰기 실패: {e}"))?;
    let run = Command::new("codex")
        .current_dir(std::env::temp_dir())
        .args(["exec", "-m", model, "-c", "model_reasoning_effort=\"low\""])
        // 사용자 설정(알림 훅 등)·규칙·세션 기록 없이, 읽기 전용으로
        .args([
            "--skip-git-repo-check",
            "--ephemeral",
            "--ignore-user-config",
            "--ignore-rules",
            "-s",
            "read-only",
        ])
        .arg("--output-schema")
        .arg(&schema_path)
        .arg("-o")
        .arg(&out_path)
        .arg(format!("{SYSTEM}\n\n{prompt}"))
        .output();
    let answer = std::fs::read_to_string(&out_path);
    let _ = std::fs::remove_file(&schema_path);
    let _ = std::fs::remove_file(&out_path);
    let out = run.map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => {
            "Codex CLI(`codex`)를 찾을 수 없습니다. 설치 후 터미널에서 `codex login`으로 로그인하세요.".to_string()
        }
        _ => format!("codex 실행 실패: {e}"),
    })?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let tail: Vec<&str> = err.trim().lines().rev().take(3).collect();
        return Err(format!(
            "Codex 오류: {} (터미널에서 `codex login` 상태를 확인하세요)",
            tail.into_iter().rev().collect::<Vec<_>>().join(" / ")
        ));
    }
    let answer = answer.map_err(|_| "Codex 응답이 비어 있습니다. 다시 시도하세요.".to_string())?;
    serde_json::from_str(&answer).map_err(|e| format!("구조화 응답 형식 오류: {e}"))
}

fn call_claude<T: DeserializeOwned>(model: &str, prompt: &str, schema: Value) -> Result<T, String> {
    let out = Command::new("claude")
        // 프로젝트 CLAUDE.md 등을 읽지 않도록 임시 폴더에서 실행
        .current_dir(std::env::temp_dir())
        .args(["-p", prompt, "--output-format", "json", "--json-schema", &schema.to_string()])
        .args(["--system-prompt", SYSTEM, "--tools", "", "--strict-mcp-config"])
        .args(["--model", model, "--no-session-persistence", "--effort", "low"])
        .output()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => {
                "Claude Code CLI(`claude`)를 찾을 수 없습니다. 설치 후 터미널에서 `claude`로 로그인하세요.".to_string()
            }
            _ => format!("claude 실행 실패: {e}"),
        })?;

    let v: Value = serde_json::from_slice(&out.stdout).map_err(|_| {
        let err = String::from_utf8_lossy(&out.stderr);
        format!("claude 응답을 읽을 수 없습니다: {}", err.trim())
    })?;
    // --output-format json 은 결과 객체 하나 또는 이벤트 배열을 준다 → 마지막 "result" 이벤트를 쓴다.
    let result = match &v {
        Value::Array(events) => events
            .iter()
            .rev()
            .find(|e| e["type"] == "result")
            .unwrap_or(&Value::Null),
        _ => &v,
    };
    if result["is_error"] == true || result.is_null() {
        let msg = result["result"].as_str().unwrap_or("알 수 없는 오류");
        return Err(format!(
            "Claude 오류: {msg} (터미널에서 `claude` 로그인 상태를 확인하세요)"
        ));
    }
    if let Some(m) = result["modelUsage"]
        .as_object()
        .and_then(|m| m.keys().next())
    {
        *CLAUDE_MODEL.lock().unwrap() = Some(m.clone());
    }
    serde_json::from_value(result["structured_output"].clone())
        .map_err(|e| format!("구조화 응답 형식 오류: {e}"))
}

#[cfg(test)]
mod tests {
    use super::{Ai, Engine};

    fn ai(engine: Engine) -> Ai {
        engine.with_model(None)
    }

    #[test]
    fn picks_latest_terra() {
        let v = serde_json::json!({"models": [
            {"slug": "gpt-6.1-sol", "visibility": "list", "priority": 1},
            {"slug": "gpt-5.6-terra", "visibility": "list", "priority": 8},
            {"slug": "gpt-6.1-terra", "visibility": "list", "priority": 3},
            {"slug": "gpt-7-terra", "visibility": "hide", "priority": 0},
        ]});
        let list = super::listed_models(&v);
        assert_eq!(
            list,
            ["gpt-6.1-sol", "gpt-6.1-terra", "gpt-5.6-terra"],
            "공개 모델만, 최신 순"
        );
        assert_eq!(super::latest_terra(&list).as_deref(), Some("gpt-6.1-terra"));
        assert!(super::listed_models(&serde_json::Value::Null).is_empty());
    }

    #[test]
    fn saved_model_must_be_in_the_engines_list() {
        let opus = "claude-opus-5-5".to_string();
        assert_eq!(Engine::Api.with_model(Some(&opus)).model, opus);
        assert_eq!(Engine::Api.with_model(None).model, "claude-haiku-5-5");
        // Claude Code 는 예전처럼 haiku 별칭이 기본
        assert_eq!(Engine::Claude.with_model(None).model, "haiku");
        // 실시간 목록을 받기 전엔 고정 목록에 없는 새 모델 이름도 믿는다
        let new = "claude-sonnet-6".to_string();
        assert_eq!(Engine::Api.with_model(Some(&new)).model, new);
        // Codex 는 자기 목록에 없는 이름이면 기본값
        let gone = "gpt-0-gone".to_string();
        assert_ne!(Engine::Codex.with_model(Some(&gone)).model, gone);
        assert_eq!(
            Engine::Api.with_model(Some(&opus)).label(),
            "Claude API · Opus 5.5"
        );
    }

    #[test]
    fn live_model_list_keeps_only_structured_output_models() {
        let page = serde_json::json!({"data": [
            {"id": "claude-opus-5-5", "display_name": "Claude Opus 5.5", "capabilities": {
                "structured_outputs": {"supported": true}, "effort": {"low": {"supported": true}}}},
            {"id": "claude-opus-4-7", "display_name": "Claude Opus 4.7", "capabilities": {
                "structured_outputs": {"supported": false}}},
            {"id": "claude-haiku-4-5", "display_name": "Claude Haiku 4.5", "capabilities": {
                "structured_outputs": {"supported": true}, "effort": {"low": {"supported": false}}}},
        ], "has_more": false});
        let m = super::live_models(&page);
        assert_eq!(m.len(), 2, "구조화 출력이 없는 모델은 뺀다");
        assert_eq!((m[0].label.as_str(), m[0].effort), ("Opus 5.5", true));
        assert_eq!((m[1].id.as_str(), m[1].effort), ("claude-haiku-4-5", false));
        assert!(super::live_models(&serde_json::Value::Null).is_empty());
    }

    #[test]
    fn api_errors_and_refusals_become_messages() {
        use serde_json::json;
        let r: Result<super::Lines, _> = super::api_result(&json!({
            "type": "error", "error": {"type": "authentication_error", "message": "invalid x-api-key"}
        }));
        assert!(r.err().unwrap().contains("거부"));
        let r: Result<super::Lines, _> = super::api_result(&json!({
            "type": "message", "stop_reason": "refusal", "content": []
        }));
        assert!(r.err().unwrap().contains("거절"));
        // thinking 블록은 건너뛰고 text 블록의 JSON 을 읽는다
        let ok: super::Lines = super::api_result(&json!({
            "type": "message", "stop_reason": "end_turn", "content": [
                {"type": "thinking", "thinking": ""},
                {"type": "text", "text": "{\"lines\":[{\"speaker\":\"\",\"en\":\"Hi.\",\"ko\":\"안녕.\"}]}"}
            ]
        }))
        .unwrap();
        assert_eq!(ok.lines[0].en, "Hi.");
        assert!(!super::valid_key("sk\" -w x") && super::valid_key("sk-ant-api03-AbC_9"));
    }

    // 실제 CLI를 부른다(사용량 소모). `cargo test -- --ignored` 로만 실행.
    #[test]
    fn unknown_or_missing_kind_does_not_break_review() {
        let f: super::Fix = serde_json::from_value(serde_json::json!(
            {"original": "a", "fix": "b", "why_ko": "c", "kind": "idiom"}
        ))
        .unwrap();
        assert_eq!((f.kind, f.line), (super::Kind::Other, 0));
        assert_eq!(super::Kind::from_key("tense"), super::Kind::Tense);
        assert_eq!(super::Kind::from_key("??"), super::Kind::Other);
    }

    #[test]
    #[ignore]
    fn live_review() {
        let line = |en: &str, ko: &str| super::Line {
            speaker: String::new(),
            en: en.into(),
            ko: ko.into(),
            start: 0.0,
            end: 0.0,
        };
        let lines = [
            line("I went to the park yesterday.", "나는 어제 공원에 갔다."),
            line("It was very sunny.", "날씨가 아주 화창했다."),
        ];
        // 2번 줄은 안 썼다 → missing 으로 보고돼야 한다
        let r =
            super::review_writing(&ai(Engine::Claude), &lines, "I go to park yesterday.").unwrap();
        assert!(r.errors.iter().any(|f| f.line == 1), "{r:?}");
        assert!(r.errors.iter().any(|f| f.line == 2), "{r:?}");
    }

    // 실제 Claude API 를 부른다(과금). 키가 있어야 한다.
    #[test]
    #[ignore]
    fn live_check_api() {
        assert!(super::refresh_models().unwrap() > 0);
        assert_eq!(
            super::check(&ai(Engine::Api)).unwrap(),
            "Claude API · Haiku 5.5"
        );
    }

    #[test]
    #[ignore]
    fn live_generate_codex() {
        let lines =
            super::generate_lines(&ai(Engine::Codex), "A2", super::Genre::Story, "a rainy day")
                .unwrap();
        assert!(
            lines.len() >= 5 && lines.iter().all(|l| !l.en.is_empty() && !l.ko.is_empty()),
            "{lines:?}"
        );
    }

    #[test]
    #[ignore]
    fn live_generate() {
        let lines = super::generate_lines(
            &ai(Engine::Claude),
            "A2",
            super::Genre::Chat,
            "ordering coffee",
        )
        .unwrap();
        let label = ai(Engine::Claude).label();
        assert!(label.contains("haiku"), "{label}");
        assert!(lines.len() >= 5, "{lines:?}");
        assert!(lines.iter().all(|l| !l.en.is_empty() && !l.ko.is_empty()));
        // 대화형: 화자 이름이 분리되고 대사에 "이름:" 접두가 남지 않아야 한다
        assert!(lines.iter().all(|l| !l.speaker.is_empty()), "{lines:?}");
        assert!(
            lines
                .iter()
                .all(|l| !l.en.starts_with(&format!("{}:", l.speaker))),
            "{lines:?}"
        );
    }
}
