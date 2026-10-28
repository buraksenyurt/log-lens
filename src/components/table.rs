use crate::app::Dataset;
use crate::parser::{LogEntry, raw_entry};
use crate::stats::human_bytes;
use leptos::prelude::*;
const PAGE_SIZE: usize = 50;

#[derive(Clone, Copy, PartialEq)]
enum SortKey {
    Order,
    Status,
    Bytes,
    Duration,
}

/// A table row representing a single log entry.
/// `key` is used by the <For> component to uniquely identify rows.
/// `entry` contains the actual log entry data.
#[derive(Clone, PartialEq)]
struct Row {
    key: (u32, usize),
    entry: LogEntry,
    // Text fields read back from the file text (entries do not store them).
    timestamp: String,
    ip: String,
    method: String,
    path: String,
}

fn status_class(status: u16) -> &'static str {
    match status {
        200..=299 => "s2",
        300..=399 => "s3",
        400..=499 => "s4",
        _ => "s5",
    }
}

#[component]
pub fn RequestTable(
    dataset: RwSignal<Option<Dataset>>,
    #[prop(into)] rows: Signal<Vec<usize>>,
) -> impl IntoView {
    let sort_key = RwSignal::new(SortKey::Order);
    let descending = RwSignal::new(false);
    let page = RwSignal::new(0usize);

    Effect::new(move |_| {
        rows.track();
        sort_key.track();
        descending.track();
        page.set(0);
    });

    let sorted = Memo::new(move |_| {
        let mut idx = rows.get();
        let key = sort_key.get();
        let desc = descending.get();
        dataset.with(|d| {
            let Some(d) = d else { return };
            let e = &d.entries;
            match key {
                SortKey::Order => {}
                SortKey::Status => idx.sort_by_key(|&i| e[i].status),
                SortKey::Bytes => idx.sort_by_key(|&i| e[i].bytes),
                SortKey::Duration => idx.sort_by(|&a, &b| {
                    let (x, y) = (
                        e[a].request_time.unwrap_or(0.0),
                        e[b].request_time.unwrap_or(0.0),
                    );
                    x.total_cmp(&y)
                }),
            }
        });
        if desc {
            idx.reverse();
        }
        idx
    });

    let page_count = move || sorted.with(|s| s.len().div_ceil(PAGE_SIZE).max(1));

    let page_rows = Memo::new(move |_| {
        let start = page.get() * PAGE_SIZE;
        dataset.with(|d| {
            let Some(d) = d else { return Vec::new() };
            sorted.with(|s| {
                s.iter()
                    .skip(start)
                    .take(PAGE_SIZE)
                    .map(|&i| {
                        let e = &d.entries[i];
                        // Only the visible rows are read back from the file text.
                        let raw = raw_entry(&d.text, e);
                        Row {
                            key: (d.id, i),
                            entry: e.clone(),
                            timestamp: raw.timestamp.to_string(),
                            ip: raw.ip.to_string(),
                            method: raw.method.to_string(),
                            path: raw.path.to_string(),
                        }
                    })
                    .collect::<Vec<_>>()
            })
        })
    });

    let sort_by = move |key: SortKey| {
        if sort_key.get_untracked() == key {
            descending.update(|d| *d = !*d);
        } else {
            sort_key.set(key);
            descending.set(key != SortKey::Order);
        }
    };

    let arrow = move |key: SortKey| {
        move || {
            if sort_key.get() != key {
                ""
            } else if descending.get() {
                " ▼"
            } else {
                " ▲"
            }
        }
    };

    view! {
        <section class="table-wrap">
            <table>
                <thead>
                    <tr>
                        <th class="sortable" on:click=move |_| sort_by(SortKey::Order)>
                            "Zaman" {arrow(SortKey::Order)}
                        </th>
                        <th>"IP"</th>
                        <th>"Method"</th>
                        <th>"Path"</th>
                        <th class="sortable num" on:click=move |_| sort_by(SortKey::Status)>
                            "Status" {arrow(SortKey::Status)}
                        </th>
                        <th class="sortable num" on:click=move |_| sort_by(SortKey::Bytes)>
                            "Boyut" {arrow(SortKey::Bytes)}
                        </th>
                        <th class="sortable num" on:click=move |_| sort_by(SortKey::Duration)>
                            "Süre" {arrow(SortKey::Duration)}
                        </th>
                    </tr>
                </thead>
                <tbody>
                    <For
                        each=move || page_rows.get()
                        key=|row| row.key
                        children=move |row| {
                            let e = row.entry;
                            let title = row.path.clone();
                            view! {
                                <tr>
                                    <td class="mono">{row.timestamp}</td>
                                    <td class="mono">{row.ip}</td>
                                    <td>{row.method}</td>
                                    <td class="path" title=title>{row.path}</td>
                                    <td class=format!("num {}", status_class(e.status))>{e.status}</td>
                                    <td class="num">{human_bytes(e.bytes)}</td>
                                    <td class="num">
                                        {e.request_time.map(|t| format!("{:.0} ms", t * 1000.0))}
                                    </td>
                                </tr>
                            }
                        }
                    />
                </tbody>
            </table>
            <div class="pager">
                <button
                    disabled=move || page.get() == 0
                    on:click=move |_| page.update(|p| *p -= 1)
                >
                    "‹ Önceki"
                </button>
                <span>{move || format!("Sayfa {} / {}", page.get() + 1, page_count())}</span>
                <button
                    disabled=move || { page.get() + 1 >= page_count() }
                    on:click=move |_| page.update(|p| *p += 1)
                >
                    "Sonraki ›"
                </button>
            </div>
        </section>
    }
}
