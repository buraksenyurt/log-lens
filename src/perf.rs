//! Thin wrappers around the browser's User Timing API (`performance.mark` / `measure`).
//!
//! The measures appear in the Firefox/Chrome profiler and are read by
//! `bench/browser.mjs`. Only call these from browser code paths: on native
//! targets (tests, benches) web-sys calls are not available.

fn performance() -> Option<web_sys::Performance> {
    web_sys::window().and_then(|w| w.performance())
}

/// `performance.now()` in milliseconds (0.0 when not available).
pub fn now() -> f64 {
    performance().map(|p| p.now()).unwrap_or(0.0)
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

/// Creates a measure with an explicit start time and duration, e.g. CPU time
/// summed over several chunks (`performance.measure(name, { start, duration })`).
pub fn measure_span(name: &str, start_ms: f64, duration_ms: f64) {
    use js_sys::{Function, Object, Reflect};
    use wasm_bindgen::JsCast;
    let Some(p) = performance() else { return };
    let options = Object::new();
    let _ = Reflect::set(&options, &"start".into(), &start_ms.into());
    let _ = Reflect::set(&options, &"duration".into(), &duration_ms.into());
    if let Ok(measure) = Reflect::get(&p, &"measure".into()).and_then(|f| f.dyn_into::<Function>().map_err(Into::into)) {
        let _ = measure.call2(&p, &name.into(), &options);
    }
}

/// Creates a measure named `name` between two existing marks.
pub fn measure(name: &str, start_mark: &str, end_mark: &str) {
    if let Some(p) = performance() {
        let _ = p.measure_with_start_mark_and_end_mark(name, start_mark, end_mark);
    }
}
