//! Parse Nginx log lines formatted in combined log format
//!
//! Expected log line format:
//! 192.168.1.10 - - [25/Oct/2026:13:55:36 +0300] "GET /api/products HTTP/1.1" 200 1534 "-" "Mozilla/5.0" 0.123
use rustc_hash::FxHashMap;

/// A parsed log entry. The request path is not stored per entry: logs usually
/// contain few distinct paths, so entries keep an index into [`PathTable`].
#[derive(Debug, Clone, PartialEq)]
pub struct LogEntry {
    pub ip: String,
    pub timestamp: String,
    pub hour: u8,
    pub method: String,
    pub path_id: u32,
    pub status: u16,
    pub bytes: u64,
    pub user_agent: String,
    pub request_time: Option<f64>,
}

/// The fields of one log line, borrowed from the input text (no allocation).
#[derive(Debug, Clone, PartialEq)]
pub struct RawEntry<'a> {
    pub ip: &'a str,
    pub timestamp: &'a str,
    pub hour: u8,
    pub method: &'a str,
    pub path: &'a str,
    pub status: u16,
    pub bytes: u64,
    pub user_agent: &'a str,
    pub request_time: Option<f64>,
}

/// Distinct request paths of a file (interning).
///
/// - `path(id)`: the full path, query string included (shown in the table)
/// - `endpoint_of(id)`: the path without its query string, used for grouping
///   (`/api/search?q=a` and `/api/search?q=b` are the same endpoint)
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PathTable {
    paths: Vec<String>,
    endpoint_of: Vec<u32>,
    endpoints: Vec<String>,
    path_ids: FxHashMap<String, u32>,
    endpoint_ids: FxHashMap<String, u32>,
}

impl PathTable {
    /// Returns the id of `path`, adding it on first sight.
    pub fn intern(&mut self, path: &str) -> u32 {
        if let Some(&id) = self.path_ids.get(path) {
            return id;
        }
        let endpoint = path.split('?').next().unwrap_or(path);
        let endpoint_id = match self.endpoint_ids.get(endpoint) {
            Some(&id) => id,
            None => {
                let id = self.endpoints.len() as u32;
                self.endpoints.push(endpoint.to_string());
                self.endpoint_ids.insert(endpoint.to_string(), id);
                id
            }
        };
        let id = self.paths.len() as u32;
        self.paths.push(path.to_string());
        self.endpoint_of.push(endpoint_id);
        self.path_ids.insert(path.to_string(), id);
        id
    }

    pub fn path(&self, id: u32) -> &str {
        &self.paths[id as usize]
    }

    pub fn endpoint_of(&self, id: u32) -> u32 {
        self.endpoint_of[id as usize]
    }

    pub fn endpoint(&self, endpoint_id: u32) -> &str {
        &self.endpoints[endpoint_id as usize]
    }

    /// Number of distinct paths (query string included).
    pub fn len(&self) -> usize {
        self.paths.len()
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Number of distinct endpoints (query string ignored).
    pub fn endpoint_count(&self) -> usize {
        self.endpoints.len()
    }

    /// All distinct paths in id order.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.paths.iter().map(String::as_str)
    }
}

#[derive(Debug, PartialEq)]
pub enum ParseError {
    MissingField(&'static str),
    InvalidNumber(&'static str),
}

/// Parse a single log line
pub fn parse_line(line: &str) -> Result<RawEntry<'_>, ParseError> {
    let line = line.trim();
    let (ip, rest) = line.split_once(' ').ok_or(ParseError::MissingField("ip"))?;
    let rest = rest
        .split_once('[')
        .ok_or(ParseError::MissingField("timestamp"))?
        .1;

    let (timestamp, rest) = rest
        .split_once(']')
        .ok_or(ParseError::MissingField("timestamp"))?;

    let hour = timestamp
        .split(':')
        .nth(1)
        .and_then(|h| h.parse::<u8>().ok())
        .ok_or(ParseError::InvalidNumber("hour"))?;

    let (request, rest) = take_quoted(rest).ok_or(ParseError::MissingField("request"))?;
    let mut parts = request.split_whitespace();
    let method = parts.next().ok_or(ParseError::MissingField("method"))?;
    let path = parts.next().ok_or(ParseError::MissingField("path"))?;
    let mut nums = rest.split_whitespace();
    let status = nums
        .next()
        .ok_or(ParseError::MissingField("status"))?
        .parse::<u16>()
        .map_err(|_| ParseError::InvalidNumber("status"))?;

    let bytes = match nums.next().ok_or(ParseError::MissingField("bytes"))? {
        "-" => 0,
        b => b
            .parse::<u64>()
            .map_err(|_| ParseError::InvalidNumber("bytes"))?,
    };

    let (_referer, rest) = take_quoted(rest).ok_or(ParseError::MissingField("referer"))?;
    let (user_agent, rest) = take_quoted(rest).ok_or(ParseError::MissingField("user_agent"))?;
    let request_time = rest.trim().parse::<f64>().ok();

    Ok(RawEntry {
        ip,
        timestamp,
        hour,
        method,
        path,
        status,
        bytes,
        user_agent,
        request_time,
    })
}

/// Returns the first "..." block and the rest of the string.
fn take_quoted(s: &str) -> Option<(&str, &str)> {
    let start = s.find('"')? + 1;
    let len = s[start..].find('"')?;
    Some((&s[start..start + len], &s[start + len + 1..]))
}

/// Parse result: successful entries and number of failed lines.
#[derive(Debug, Default, Clone)]
pub struct ParseResult {
    pub entries: Vec<LogEntry>,
    pub paths: PathTable,
    pub failed: usize,
}

impl ParseResult {
    pub fn push_line(&mut self, line: &str) {
        if line.trim().is_empty() {
            return;
        }
        match parse_line(line) {
            Ok(raw) => {
                let path_id = self.paths.intern(raw.path);
                self.entries.push(LogEntry {
                    ip: raw.ip.to_string(),
                    timestamp: raw.timestamp.to_string(),
                    hour: raw.hour,
                    method: raw.method.to_string(),
                    path_id,
                    status: raw.status,
                    bytes: raw.bytes,
                    user_agent: raw.user_agent.to_string(),
                    request_time: raw.request_time,
                });
            }
            Err(_) => self.failed += 1,
        }
    }
}

/// Parses a whole text at once (used by tests and native benchmarks).
pub fn parse_all(text: &str) -> ParseResult {
    let mut result = ParseResult::default();
    for line in text.lines() {
        result.push_line(line);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"192.168.1.10 - - [25/Oct/2026:13:55:36 +0300] "GET /api/products?id=3 HTTP/1.1" 200 1534 "-" "Mozilla/5.0 (X11; Linux x86_64)" 0.123"#;

    #[test]
    fn parses_full_line() {
        let e = parse_line(SAMPLE).unwrap();
        assert_eq!(e.ip, "192.168.1.10");
        assert_eq!(e.hour, 13);
        assert_eq!(e.method, "GET");
        assert_eq!(e.path, "/api/products?id=3");
        assert_eq!(e.status, 200);
        assert_eq!(e.bytes, 1534);
        assert_eq!(e.user_agent, "Mozilla/5.0 (X11; Linux x86_64)");
        assert_eq!(e.request_time, Some(0.123));
    }

    #[test]
    fn request_time_is_optional() {
        let line = SAMPLE.trim_end_matches(" 0.123");
        assert_eq!(parse_line(line).unwrap().request_time, None);
    }

    #[test]
    fn dash_bytes_means_zero() {
        let line = SAMPLE.replace(" 1534 ", " - ");
        assert_eq!(parse_line(&line).unwrap().bytes, 0);
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_line("bu bir log satırı değil").is_err());
    }

    #[test]
    fn parse_all_counts_failures() {
        let text = format!("{SAMPLE}\n\nbozuk satır\n{SAMPLE}\n");
        let r = parse_all(&text);
        assert_eq!(r.entries.len(), 2);
        assert_eq!(r.failed, 1);
    }

    #[test]
    fn paths_are_interned() {
        let a = SAMPLE;
        let b = SAMPLE.replace("/api/products?id=3", "/api/products?id=4");
        let c = SAMPLE.replace("/api/products?id=3", "/login");
        let r = parse_all(&format!("{a}\n{b}\n{a}\n{c}\n"));

        let ids: Vec<u32> = r.entries.iter().map(|e| e.path_id).collect();
        assert_eq!(ids, vec![0, 1, 0, 2]);
        assert_eq!(r.paths.len(), 3);
        assert_eq!(r.paths.path(1), "/api/products?id=4");
        // Query strings are ignored for endpoints: id=3 and id=4 share one.
        assert_eq!(r.paths.endpoint_count(), 2);
        assert_eq!(r.paths.endpoint_of(0), r.paths.endpoint_of(1));
        assert_eq!(r.paths.endpoint(r.paths.endpoint_of(2)), "/login");
    }
}
