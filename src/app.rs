use crate::components::{
    filters::FilterBar,
    insights::{HourlyChart, TopEndpoints},
    stats::StatsPanel,
    table::RequestTable,
    upload::FileDrop,
};
use crate::filter::Filters;
use crate::i18n::{I18n, LanguageSelect};
use crate::loader::load_file;
use crate::parser::{LogEntry, Tables};
use crate::perf;
use crate::stats::{hourly, summarize, top_endpoints};
use leptos::prelude::*;
use leptos::task::spawn_local;

// Log file that has been loaded and parsed
#[derive(Clone)]
pub struct Dataset {
    pub id: u32,
    pub name: String,
    /// The file content. Entries point into it; the table reads the visible
    /// rows' text from here instead of every entry keeping its own copies.
    pub text: String,
    pub entries: Vec<LogEntry>,
    /// Interned IPs, methods and paths; entries refer to them by id.
    pub tables: Tables,
    pub failed: usize,
    pub parse_ms: f64,
}

#[component]

pub fn App() -> impl IntoView {
    let dataset = RwSignal::new(None::<Dataset>);
    let load_count = StoredValue::new(0u32);
    let progress = RwSignal::new(None::<f64>);
    let i18n = I18n::new();
    provide_context(i18n);
    // (file name, reason); formatted in the current language when shown.
    let error = RwSignal::new(None::<(String, String)>);
    let on_file = Callback::new(move |file: web_sys::File| {
        if progress.get_untracked().is_some() {
            return; // zaten bir dosya işleniyor
        }

        progress.set(Some(0.0));
        error.set(None);
        spawn_local(async move {
            let name = file.name();
            // Reading and parsing overlap: lines are parsed as the file streams in.
            match load_file(&file, |p| progress.set(Some(p))).await {
                Ok(loaded) => {
                    load_count.update_value(|c| *c += 1);
                    dataset.set(Some(Dataset {
                        id: load_count.get_value(),
                        name,
                        text: loaded.text,
                        entries: loaded.result.entries,
                        tables: loaded.result.tables,
                        failed: loaded.result.failed,
                        parse_ms: loaded.parse_ms,
                    }));
                }
                Err(reason) => error.set(Some((name, reason))),
            }
            progress.set(None);
            perf::notify("loglens:loaded");
        });
    });

    let filters = Filters::new();
    provide_context(filters);
    let filtered = Memo::new(move |_| {
        let criteria = filters.criteria();
        dataset.with(|d| match d {
            Some(d) => {
                let matcher = criteria.matcher(&d.tables);
                d.entries
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| matcher.matches(e))
                    .map(|(i, _)| i)
                    .collect::<Vec<usize>>()
            }
            None => Vec::new(),
        })
    });

    let summary = Memo::new(move |_| {
        dataset.with(|d| match d {
            Some(d) => filtered.with(|idx| summarize(idx.iter().map(|&i| &d.entries[i]))),
            None => Default::default(),
        })
    });

    let hours = Memo::new(move |_| {
        dataset.with(|d| match d {
            Some(d) => filtered.with(|idx| hourly(idx.iter().map(|&i| &d.entries[i]))),
            None => Default::default(),
        })
    });

    let endpoints = Memo::new(move |_| {
        dataset.with(|d| match d {
            Some(d) => filtered.with(|idx| top_endpoints(idx.iter().map(|&i| &d.entries[i]), &d.tables.paths, 8)),
            None => Vec::new(),
        })
    });

    view! {
        <header>
            <div class="header-row">
                <h1>"Log Lens"</h1>
                <LanguageSelect />
            </div>
            <p>{move || i18n.t().subtitle}</p>
        </header>
        <main>
            <FileDrop on_file=on_file />
            {move || {
                error
                    .get()
                    .map(|(file, reason)| {
                        let reason = if reason.is_empty() { i18n.t().unknown_error.to_string() } else { reason };
                        view! { <p class="error">{i18n.lang().read_error(&file, &reason)}</p> }
                    })
            }}
            {move || progress.get().map(|p| view! {
                <div class="progress">
                    <div class="progress-bar" style:width=format!("{:.0}%", p * 100.0)></div>
                    <span>{i18n.lang().parsing_progress(p)}</span>
                </div>
            })}
            <Show when=move || dataset.with(|d| d.is_some())>
                {move || dataset.with(|d| d.as_ref().map(|d| view! {
                    <p class="meta">
                        <b>{d.name.clone()}</b>
                        {i18n.lang().file_summary(d.entries.len(), d.failed, d.parse_ms)}
                    </p>
                }))}
                <FilterBar matched=Signal::derive(move || filtered.with(|v| v.len())) />
                <StatsPanel summary=summary />
                <section class="insights">
                    <HourlyChart buckets=hours />
                    <TopEndpoints items=endpoints />
                </section>
                <RequestTable dataset=dataset rows=filtered />
            </Show>
        </main>
    }
}
