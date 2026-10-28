//! Filtering criteria and matching logic.
use crate::parser::{LogEntry, PathTable};
use leptos::prelude::*;

/// Raw fitering criteria that can be applied to log entries.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Criteria {
    /// 2, 3, 4, 5 => 2xx, 3xx...
    pub status_class: Option<u16>,
    pub method: Option<String>,
    pub path_contains: String,
    pub min_ms: Option<f64>,
}

impl Criteria {
    /// Prepares the criteria for one dataset. The path condition is evaluated
    /// once per distinct path here, instead of once per entry in `matches`.
    pub fn matcher(&self, paths: &PathTable) -> Matcher<'_> {
        let path_ok = (!self.path_contains.is_empty()).then(|| {
            paths
                .iter()
                .map(|p| p.contains(&self.path_contains))
                .collect()
        });
        Matcher {
            criteria: self,
            path_ok,
        }
    }
}

/// [`Criteria`] bound to a dataset's [`PathTable`].
pub struct Matcher<'a> {
    criteria: &'a Criteria,
    /// path_ok[path_id]: does that path contain the searched text?
    path_ok: Option<Vec<bool>>,
}

impl Matcher<'_> {
    pub fn matches(&self, e: &LogEntry) -> bool {
        let c = self.criteria;
        if let Some(class) = c.status_class {
            if e.status / 100 != class {
                return false;
            }
        }

        if let Some(ok) = &self.path_ok {
            if !ok[e.path_id as usize] {
                return false;
            }
        }

        if let Some(m) = &c.method {
            if &e.method != m {
                return false;
            }
        }

        if let Some(min) = c.min_ms {
            match e.request_time {
                Some(t) if t * 1000.0 >= min => {}
                _ => return false,
            }
        }
        true
    }
}

/// Reactive filter state tied to the form fields in the UI.
/// Provides a convenient way to manage and reset filter state in the UI.
#[derive(Clone, Copy)]
pub struct Filters {
    pub status: RwSignal<String>,
    pub method: RwSignal<String>,
    pub path: RwSignal<String>,
    pub min_ms: RwSignal<String>,
}

impl Filters {
    pub fn new() -> Self {
        Self {
            status: RwSignal::new(String::new()),
            method: RwSignal::new(String::new()),
            path: RwSignal::new(String::new()),
            min_ms: RwSignal::new(String::new()),
        }
    }

    /// Converts the current reactive filter state into a plain `Criteria` instance.
    pub fn criteria(&self) -> Criteria {
        Criteria {
            status_class: self.status.get().parse().ok(),
            method: Some(self.method.get()).filter(|m| !m.is_empty()),
            path_contains: self.path.get().trim().to_string(),
            min_ms: self.min_ms.get().parse().ok(),
        }
    }

    pub fn reset(&self) {
        self.status.set(String::new());
        self.method.set(String::new());
        self.path.set(String::new());
        self.min_ms.set(String::new());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ParseResult;

    /// Parses one line per (method, path, status, request time) tuple.
    fn parse(rows: &[(&str, &str, u16, f64)]) -> ParseResult {
        let mut r = ParseResult::default();
        for (method, path, status, rt) in rows {
            r.push_line(&format!(
                r#"1.1.1.1 - - [25/Oct/2026:10:00:00 +0300] "{method} {path} HTTP/1.1" {status} 1 "-" "ua" {rt}"#
            ));
        }
        r
    }

    #[test]
    fn empty_criteria_matches_all() {
        let r = parse(&[("GET", "/", 200, 0.1)]);
        assert!(Criteria::default().matcher(&r.paths).matches(&r.entries[0]));
    }

    #[test]
    fn combined_criteria() {
        let c = Criteria {
            status_class: Some(5),
            method: Some("POST".into()),
            path_contains: "checkout".into(),
            min_ms: Some(500.0),
        };

        let r = parse(&[
            ("POST", "/api/checkout", 502, 0.9),
            ("POST", "/api/checkout", 200, 0.9),
            ("GET", "/api/checkout", 502, 0.9),
            ("POST", "/api/cart", 502, 0.9),
            ("POST", "/api/checkout", 502, 0.1),
        ]);
        let m = c.matcher(&r.paths);
        let results: Vec<bool> = r.entries.iter().map(|e| m.matches(e)).collect();
        assert_eq!(results, vec![true, false, false, false, false]);
    }
}
