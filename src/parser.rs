//! Parse Nginx log lines formatted in combined log format
//!
//! Expected log line format:
//! 192.168.1.10 - - [25/Oct/2026:13:55:36 +0300] "GET /api/products HTTP/1.1" 200 1534 "-" "Mozilla/5.0" 0.123
use rustc_hash::FxHashMap;

/// A parsed log entry. Text fields are not stored per entry: logs repeat the
/// same IPs, methods, paths, user agents and dates over and over, so entries
/// only keep small ids into the file's [`Tables`]. The entry stays small
/// (no heap allocation), which also makes scanning 500k entries cache friendly.
#[derive(Debug, Clone, PartialEq)]
pub struct LogEntry {
    pub ip_id: u32,
    pub date_id: u32,
    pub zone_id: u32,
    /// Time of day in seconds (0..86400).
    pub seconds: u32,
    pub hour: u8,
    pub method_id: u32,
    pub path_id: u32,
    pub status: u16,
    pub bytes: u64,
    pub agent_id: u32,
    pub request_time: Option<f64>,
}

/// The fields of one log line, borrowed from the input text (no allocation).
#[derive(Debug, Clone, PartialEq)]
pub struct RawEntry<'a> {
    pub ip: &'a str,
    pub timestamp: &'a str,
    /// Date part of the timestamp, e.g. `25/Oct/2026`.
    pub date: &'a str,
    /// Time of day in seconds.
    pub seconds: u32,
    /// Time zone part of the timestamp, e.g. `+0300`.
    pub zone: &'a str,
    pub hour: u8,
    pub method: &'a str,
    pub path: &'a str,
    pub status: u16,
    pub bytes: u64,
    pub user_agent: &'a str,
    pub request_time: Option<f64>,
}

/// Stores each distinct string once and hands out stable `u32` ids.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Interner {
    values: Vec<String>,
    ids: FxHashMap<String, u32>,
}

impl Interner {
    /// Returns the id of `value`, adding it on first sight. Looking up an
    /// existing value does not allocate.
    pub fn intern(&mut self, value: &str) -> u32 {
        if let Some(&id) = self.ids.get(value) {
            return id;
        }
        let id = self.values.len() as u32;
        self.values.push(value.to_string());
        self.ids.insert(value.to_string(), id);
        id
    }

    pub fn get(&self, id: u32) -> &str {
        &self.values[id as usize]
    }

    /// The id of an already interned value.
    pub fn id_of(&self, value: &str) -> Option<u32> {
        self.ids.get(value).copied()
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// All values in id order.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.values.iter().map(String::as_str)
    }
}

/// Distinct request paths of a file.
///
/// - `path(id)`: the full path, query string included (shown in the table)
/// - `endpoint_of(id)`: the path without its query string, used for grouping
///   (`/api/search?q=a` and `/api/search?q=b` are the same endpoint)
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PathTable {
    paths: Interner,
    endpoints: Interner,
    endpoint_of: Vec<u32>,
}

impl PathTable {
    /// Returns the id of `path`, adding it on first sight.
    pub fn intern(&mut self, path: &str) -> u32 {
        if let Some(id) = self.paths.id_of(path) {
            return id;
        }
        let endpoint = path.split('?').next().unwrap_or(path);
        let endpoint_id = self.endpoints.intern(endpoint);
        self.endpoint_of.push(endpoint_id);
        self.paths.intern(path)
    }

    pub fn path(&self, id: u32) -> &str {
        self.paths.get(id)
    }

    pub fn endpoint_of(&self, id: u32) -> u32 {
        self.endpoint_of[id as usize]
    }

    pub fn endpoint(&self, endpoint_id: u32) -> &str {
        self.endpoints.get(endpoint_id)
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
        self.paths.iter()
    }
}

/// All interned strings of one file. Entries refer to them by id.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tables {
    pub paths: PathTable,
    pub ips: Interner,
    pub methods: Interner,
    pub agents: Interner,
    pub dates: Interner,
    pub zones: Interner,
}

impl Tables {
    pub fn ip(&self, e: &LogEntry) -> &str {
        self.ips.get(e.ip_id)
    }

    pub fn method(&self, e: &LogEntry) -> &str {
        self.methods.get(e.method_id)
    }

    pub fn path(&self, e: &LogEntry) -> &str {
        self.paths.path(e.path_id)
    }

    pub fn user_agent(&self, e: &LogEntry) -> &str {
        self.agents.get(e.agent_id)
    }

    /// Rebuilds the original nginx timestamp, e.g. `25/Oct/2026:13:55:36 +0300`.
    pub fn timestamp(&self, e: &LogEntry) -> String {
        let (h, m, s) = (e.seconds / 3600, e.seconds / 60 % 60, e.seconds % 60);
        let zone = self.zones.get(e.zone_id);
        let date = self.dates.get(e.date_id);
        if zone.is_empty() {
            format!("{date}:{h:02}:{m:02}:{s:02}")
        } else {
            format!("{date}:{h:02}:{m:02}:{s:02} {zone}")
        }
    }
}

/// Splits `25/Oct/2026:13:55:36 +0300` into (`25/Oct/2026`, seconds of day, `+0300`).
fn split_timestamp(ts: &str) -> Option<(&str, u32, &str)> {
    let (date, rest) = ts.split_once(':')?;
    let (time, zone) = rest.split_once(' ').unwrap_or((rest, ""));
    let mut parts = time.split(':').map(|p| p.parse::<u32>().ok());
    let (h, m, s) = (parts.next()??, parts.next()??, parts.next()??);
    if h > 23 || m > 59 || s > 60 {
        return None;
    }
    Some((date, h * 3600 + m * 60 + s, zone))
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

    let (date, seconds, zone) =
        split_timestamp(timestamp).ok_or(ParseError::InvalidNumber("time"))?;
    let hour = (seconds / 3600) as u8;

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
        date,
        seconds,
        zone,
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
    pub tables: Tables,
    pub failed: usize,
}

impl ParseResult {
    pub fn push_line(&mut self, line: &str) {
        if line.trim().is_empty() {
            return;
        }
        match parse_line(line) {
            Ok(raw) => {
                let t = &mut self.tables;
                self.entries.push(LogEntry {
                    ip_id: t.ips.intern(raw.ip),
                    date_id: t.dates.intern(raw.date),
                    zone_id: t.zones.intern(raw.zone),
                    seconds: raw.seconds,
                    hour: raw.hour,
                    method_id: t.methods.intern(raw.method),
                    path_id: t.paths.intern(raw.path),
                    status: raw.status,
                    bytes: raw.bytes,
                    agent_id: t.agents.intern(raw.user_agent),
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
        assert_eq!(e.date, "25/Oct/2026");
        assert_eq!(e.seconds, 13 * 3600 + 55 * 60 + 36);
        assert_eq!(e.zone, "+0300");
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
        let paths = &r.tables.paths;
        assert_eq!(paths.len(), 3);
        assert_eq!(paths.path(1), "/api/products?id=4");
        // Query strings are ignored for endpoints: id=3 and id=4 share one.
        assert_eq!(paths.endpoint_count(), 2);
        assert_eq!(paths.endpoint_of(0), paths.endpoint_of(1));
        assert_eq!(paths.endpoint(paths.endpoint_of(2)), "/login");
    }

    #[test]
    fn entries_round_trip_through_tables() {
        let r = parse_all(&format!("{SAMPLE}\n{SAMPLE}\n"));
        let (t, e) = (&r.tables, &r.entries[1]);
        assert_eq!(t.ip(e), "192.168.1.10");
        assert_eq!(t.method(e), "GET");
        assert_eq!(t.path(e), "/api/products?id=3");
        assert_eq!(t.user_agent(e), "Mozilla/5.0 (X11; Linux x86_64)");
        assert_eq!(t.timestamp(e), "25/Oct/2026:13:55:36 +0300");
        // The same strings are stored once.
        assert_eq!((t.ips.len(), t.agents.len(), t.dates.len()), (1, 1, 1));
    }

    #[test]
    fn rejects_invalid_time() {
        let line = SAMPLE.replace("13:55:36", "25:00:00");
        assert!(parse_line(&line).is_err());
    }
}
