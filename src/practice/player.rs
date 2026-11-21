// 브라우저 속 유튜브 플레이어를 앱에서 조작한다.
// 127.0.0.1에 작은 HTTP 서버를 띄워 플레이어 페이지(player.html)를 주고, 페이지는 /cmd를 0.2초마다 읽는다. 명령은 둘:
// - segment: "이 영상의 start~end 구간을 재생하고 end에서 멈춰라" (따라치기)
// - watch:   "이 영상을 처음부터 보여 주고 /subs 의 자막을 함께 띄워라" (영상 보기)
// 의존성 없이 std만 쓴다.
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const PAGE: &str = include_str!("player.html");
/// 고정 포트. 브라우저 저장소(localStorage: 자막 토글 · 이어 보기 위치)는 주소(포트 포함)마다 따로라서,
/// 포트가 매번 바뀌면 지난번 설정을 못 읽는다. 다른 프로그램이 쓰고 있으면 아무 포트나 쓴다(그때는 설정이 새로 시작).
const PORT: u16 = 38917;

#[derive(Default)]
struct Cmd {
    seq: u64,
    /// "segment" 또는 "watch"
    mode: &'static str,
    id: String,
    start: f64,
    end: f64,
    rate: f64,
    /// 자막이 바뀔 때마다 올라간다. 페이지는 이 값이 바뀌면 /subs 를 다시 받는다.
    subs_ver: u64,
    /// 보기 모드 자막 JSON 배열: [{start, end, en, ko}]
    subs: String,
}

pub struct Player {
    port: u16,
    cmd: Arc<Mutex<Cmd>>,
}

impl Player {
    pub fn start() -> Result<Self, String> {
        let listener = TcpListener::bind(("127.0.0.1", PORT))
            .or_else(|_| TcpListener::bind("127.0.0.1:0"))
            .map_err(|e| format!("플레이어 서버 시작 실패: {e}"))?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let cmd = Arc::new(Mutex::new(Cmd {
            mode: "segment",
            rate: 1.0,
            subs: "[]".into(),
            ..Cmd::default()
        }));
        let shared = cmd.clone();
        std::thread::spawn(move || {
            // 연결마다 스레드 — 브라우저가 미리 열어 두고 요청을 안 보내는 소켓이 다른 요청을 막지 않게
            for stream in listener.incoming().flatten() {
                let shared = shared.clone();
                std::thread::spawn(move || serve(stream, &shared, port));
            }
        });
        Ok(Self { port, cmd })
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.port)
    }

    /// 구간 재생. 같은 구간을 다시 보내도 seq가 바뀌므로 처음부터 다시 재생된다.
    pub fn play(&self, id: &str, start: f64, end: f64, rate: f64) {
        let mut c = self.cmd.lock().unwrap();
        c.seq += 1;
        c.mode = "segment";
        c.id = id.to_string();
        // 문장 앞뒤가 잘리지 않게 약간 여유를 둔다
        c.start = (start - 0.25).max(0.0);
        c.end = end + 0.35;
        c.rate = rate;
    }

    /// 영상 보기: 처음부터 이어서 재생하고 자막을 함께 띄운다.
    pub fn watch(&self, id: &str) {
        let mut c = self.cmd.lock().unwrap();
        c.seq += 1;
        c.mode = "watch";
        c.id = id.to_string();
        c.start = 0.0;
        c.end = 0.0;
    }

    /// 보기 모드 자막을 바꾼다(번역이 한 세트씩 채워질 때마다).
    pub fn set_subs(&self, subs: serde_json::Value) {
        let mut c = self.cmd.lock().unwrap();
        c.subs = subs.to_string();
        c.subs_ver += 1;
    }
}

/// Host 헤더가 이 서버(127.0.0.1 · localhost + 포트)인지.
/// DNS 리바인딩 막기: 악성 사이트가 자기 도메인을 127.0.0.1 로 돌려도 Host 는 그 도메인이라 거절된다.
fn host_ok(req: &str, port: u16) -> bool {
    req.lines()
        .find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.trim().eq_ignore_ascii_case("host").then(|| v.trim())
        })
        .is_some_and(|h| h == format!("127.0.0.1:{port}") || h == format!("localhost:{port}"))
}

fn serve(mut s: TcpStream, cmd: &Mutex<Cmd>, port: u16) -> std::io::Result<()> {
    s.set_read_timeout(Some(Duration::from_secs(1)))?;
    // 헤더 끝(빈 줄)까지 읽는다. 덜 읽고 닫으면 상대가 연결 리셋을 받는다.
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") && buf.len() < 16 * 1024 {
        let n = s.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let req = String::from_utf8_lossy(&buf);
    if !host_ok(&req, port) {
        return write!(
            s,
            "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
    }
    let path = req.split_whitespace().nth(1).unwrap_or("/");
    let (ctype, body) = if path.starts_with("/cmd") {
        let c = cmd.lock().unwrap();
        let json = serde_json::json!({"seq": c.seq, "mode": c.mode, "id": c.id, "start": c.start,
            "end": c.end, "rate": c.rate, "subs_ver": c.subs_ver});
        ("application/json", json.to_string())
    } else if path.starts_with("/subs") {
        ("application/json", cmd.lock().unwrap().subs.clone())
    } else {
        ("text/html; charset=utf-8", PAGE.to_string())
    };
    write!(
        s,
        "HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get(port: u16, path: &str) -> String {
        get_as(port, path, &format!("127.0.0.1:{port}"))
    }

    fn get_as(port: u16, path: &str, host: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(s, "GET {path} HTTP/1.1\r\nHost: {host}\r\n\r\n").unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        out
    }

    #[test]
    fn serves_page_and_commands() {
        let p = Player::start().unwrap();
        assert!(get(p.port, "/").contains("YT.Player"));
        p.play("abc", 10.0, 12.0, 0.75);
        let r = get(p.port, "/cmd");
        let body: serde_json::Value =
            serde_json::from_str(r.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(body["seq"], 1);
        assert_eq!(body["id"], "abc");
        assert_eq!(body["start"], 9.75);
        assert_eq!(body["rate"], 0.75);
        assert_eq!(body["mode"], "segment");

        // 보기 모드: 명령이 바뀌고, 자막을 바꾸면 버전이 올라간다
        p.watch("xyz");
        p.set_subs(serde_json::json!([{"start": 1.0, "end": 2.0, "en": "Hi.", "ko": "안녕."}]));
        let r = get(p.port, "/cmd");
        let body: serde_json::Value =
            serde_json::from_str(r.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(
            (body["mode"].as_str(), body["seq"].as_u64()),
            (Some("watch"), Some(2))
        );
        assert_eq!(body["subs_ver"], 1);
        let r = get(p.port, "/subs");
        assert!(r.contains("안녕."), "{r}");
        assert!(get_as(p.port, "/subs", &format!("localhost:{}", p.port)).contains("안녕."));

        // 다른 Host(DNS 리바인딩)는 거절
        for host in ["evil.example:38917", "127.0.0.1", "x"] {
            let r = get_as(p.port, "/subs", host);
            assert!(
                r.starts_with("HTTP/1.1 403") && !r.contains("안녕."),
                "{host}: {r}"
            );
        }
    }
}
