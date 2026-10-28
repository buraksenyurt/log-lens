//! Thin wrappers around the browser's User Timing API (`performance.mark` / `measure`).
//!
//! The measures appear in the Firefox/Chrome profiler and are read by
//! `bench/browser.mjs`. Only call these from browser code paths: on native
//! targets (tests, benches) web-sys calls are not available.

fn performance() -> Option<web_sys::Performance> {
    web_sys::window().and_then(|w| w.performance())
}

pub fn mark(name: &str) {
    if let Some(p) = performance() {
        let _ = p.mark(name);
    }
}

/// Dispatches a plain DOM event on `window` so scripts (benchmarks, tests) can
/// react the moment something happens, without polling.
pub fn notify(event_name: &str) {
    if let (Some(window), Ok(event)) = (web_sys::window(), web_sys::Event::new(event_name)) {
        let _ = window.dispatch_event(&event);
    }
}

/// Creates a measure named `name` between two existing marks.
pub fn measure(name: &str, start_mark: &str, end_mark: &str) {
    if let Some(p) = performance() {
        let _ = p.measure_with_start_mark_and_end_mark(name, start_mark, end_mark);
    }
}
