//! Reads a file into WASM memory in slices and parses each slice while the
//! browser is already reading the next one.
//!
//! `File.arrayBuffer()` on the whole file means: wait for all bytes, then copy,
//! then parse. Here the file is read in `Blob.slice()`s growing from 1 MB to 16 MB.
//! The read of slice k+1 is started before slice k is parsed; the browser reads
//! files off the main thread, so reading and parsing overlap.

use gloo_timers::future::TimeoutFuture;
use js_sys::Uint8Array;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::File;

use crate::parser::{ParseResult, parse_all};
use crate::perf;

/// Size of the first read; later reads grow with the data already read.
const FIRST_SLICE_BYTES: usize = 1024 * 1024;
/// Upper bound for one read.
const MAX_SLICE_BYTES: usize = 16 * 1024 * 1024;

/// Yield to the browser (repaint) after this many slices.
const YIELD_EVERY_SLICES: usize = 2;

pub struct Loaded {
    /// The whole file content; entries refer to it by byte offset.
    pub text: String,
    pub result: ParseResult,
    /// Time spent parsing (CPU), excluding the time spent waiting for data.
    pub parse_ms: f64,
}

/// Reads and parses `file`. `progress` receives values in 0.0..=1.0.
///
/// Records the User Timing marks/measures used by the benchmarks:
/// `loglens:read` (until the last slice arrived), `loglens:copy` and
/// `loglens:parse` (summed over all chunks) and the `loglens:ready` mark.
pub async fn load_file(file: &File, mut progress: impl FnMut(f64)) -> Result<Loaded, String> {
    perf::mark("loglens:read:start");
    let start = perf::now();
    let size = file.size() as usize;

    // One allocation for the whole file: no re-allocation (and copying) while
    // growing, and entry offsets stay valid.
    let mut buf: Vec<u8> = Vec::with_capacity(size);
    let mut result = ParseResult::default();
    // buf[..parsed] has been parsed; `parsed` is always at the start of a line.
    let mut parsed = 0;
    // Becomes false if a chunk is not valid UTF-8; the file is then parsed at
    // the end, after a lossy conversion.
    let mut valid_utf8 = true;
    let (mut copy_ms, mut parse_ms) = (0.0, 0.0);

    let read_slice = |from: usize| -> Result<JsFuture, String> {
        // Small first slices so parsing can start early, then larger ones:
        // every read has a fixed cost, so fewer reads are cheaper.
        let len = (from / 2).clamp(FIRST_SLICE_BYTES, MAX_SLICE_BYTES);
        let to = (from + len).min(size);
        let blob = file.slice_with_f64_and_f64(from as f64, to as f64).map_err(describe)?;
        Ok(JsFuture::from(blob.array_buffer()))
    };
    let mut slices = 0;
    let mut next = (size > 0).then(|| read_slice(0)).transpose()?;
    while let Some(pending) = next.take() {
        let chunk = Uint8Array::new(&pending.await.map_err(describe)?);
        if chunk.length() == 0 {
            break; // the file got shorter while reading
        }
        let old_len = buf.len();
        // Start reading the next slice before this one is parsed.
        let following = old_len + chunk.length() as usize;
        if following < size {
            next = Some(read_slice(following)?);
        }

        // JS -> WASM: copy the slice to the end of the buffer.
        let t = perf::now();
        buf.resize(following, 0);
        chunk.copy_to(&mut buf[old_len..]);
        copy_ms += perf::now() - t;

        // Parse the lines that are complete now.
        if valid_utf8 {
            if let Some(nl) = memchr::memrchr(b'\n', &buf[old_len..]) {
                let end = old_len + nl + 1;
                let t = perf::now();
                valid_utf8 = parse_segment(&mut result, &buf, parsed, end);
                parse_ms += perf::now() - t;
                parsed = end;
            }
        }

        progress(buf.len() as f64 / size as f64);
        // If the next slice is already there, awaiting it does not give the
        // browser a chance to repaint; yield now and then so the progress bar
        // moves. Not after every slice: each timeout costs at least ~4 ms.
        slices += 1;
        if slices % YIELD_EVERY_SLICES == 0 {
            TimeoutFuture::new(0).await;
        }
    }
    perf::mark("loglens:read:end");
    perf::measure("loglens:read", "loglens:read:start", "loglens:read:end");

    let t = perf::now();
    let (text, result) = if valid_utf8 && parse_segment(&mut result, &buf, parsed, buf.len()) {
        // SAFETY: buf[0..len] was validated as UTF-8 by parse_segment, in
        // consecutive segments that end right after a '\n' (always a char boundary).
        (unsafe { String::from_utf8_unchecked(buf) }, result)
    } else {
        let text = String::from_utf8_lossy(&buf).into_owned();
        let result = parse_all(&text);
        (text, result)
    };
    parse_ms += perf::now() - t;

    perf::measure_span("loglens:copy", start, copy_ms);
    perf::measure_span("loglens:parse", start, parse_ms);
    perf::mark("loglens:ready");
    Ok(Loaded { text, result, parse_ms })
}

/// Parses `buf[start..end]` (whole lines). Returns false if it is not valid UTF-8.
fn parse_segment(result: &mut ParseResult, buf: &[u8], start: usize, end: usize) -> bool {
    let Ok(segment) = std::str::from_utf8(&buf[start..end]) else {
        return false;
    };
    for line in segment.lines() {
        let offset = start + (line.as_ptr() as usize - segment.as_ptr() as usize);
        result.push_line_at(offset, line);
    }
    true
}

fn describe(e: JsValue) -> String {
    e.as_string()
        .or_else(|| js_sys::Error::try_from(e).ok().map(|e| String::from(e.message())))
        .unwrap_or_default() // empty: the UI shows its own "unknown error" text
}
