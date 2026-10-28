use crate::filter::Filters;
use crate::i18n::use_i18n;
use crate::stats::{EndpointStat, HourBucket};
use leptos::prelude::*;

/// 24 hour request histogram (pure CSS bars).
#[component]
pub fn HourlyChart(#[prop(into)] buckets: Signal<[HourBucket; 24]>) -> impl IntoView {
    let i18n = use_i18n();
    let max = move || {
        buckets
            .read()
            .iter()
            .map(|b| b.total)
            .max()
            .unwrap_or(0)
            .max(1)
    };
    view! {
        <div class="panel">
            <h3>{move || i18n.t().hourly}</h3>
            <div class="bars">
                {(0..24)
                    .map(|h| {
                        let bucket = move || buckets.read()[h];
                        let height = move || bucket().total as f64 * 100.0 / max() as f64;
                        let err_height = move || {
                            let b = bucket();
                            if b.total == 0 { 0.0 } else { b.errors as f64 * 100.0 / b.total as f64 }
                        };
                        view! {
                            <div
                                class="bar"
                                title=move || {
                                    let b = bucket();
                                    i18n.lang().hour_tooltip(h, b.total, b.errors)
                                }
                            >
                                <div class="fill" style:height=move || format!("{}%", height())>
                                    <div class="err" style:height=move || format!("{}%", err_height())></div>
                                </div>
                                <span>{format!("{h:02}")}</span>
                            </div>
                        }
                    })
                    .collect_view()}
            </div>
        </div>
    }
}

/// Most busy endpoints. Clicking a row fills the path filter.
#[component]
pub fn TopEndpoints(#[prop(into)] items: Signal<Vec<EndpointStat>>) -> impl IntoView {
    let filters = expect_context::<Filters>();
    let i18n = use_i18n();
    view! {
        <div class="panel">
            <h3>{move || i18n.t().top_endpoints}</h3>
            <table class="compact">
                <thead>
                    <tr>
                        <th>{move || i18n.t().path}</th>
                        <th class="num">{move || i18n.t().requests}</th>
                        <th class="num">"5xx"</th>
                        <th class="num">{move || i18n.t().avg_short}</th>
                    </tr>
                </thead>
                <tbody>
                    {move || {
                        // Reading the language here re-renders the rows when it changes.
                        let lang = i18n.lang();
                        let t = lang.texts();
                        items
                            .get()
                            .into_iter()
                            .map(|item| {
                                let path = item.path.clone();
                                let selected_path = item.path.clone();
                                let selected = move || {
                                    filters.endpoint.with(|e| e.as_deref() == Some(selected_path.as_str()))
                                };
                                view! {
                                    <tr
                                        class="clickable"
                                        class:selected=selected
                                        title=t.filter_by_endpoint
                                        on:click=move |_| filters.toggle_endpoint(path.clone())
                                    >
                                        <td class="mono">{item.path}</td>
                                        <td class="num">{lang.int(item.count as u64)}</td>
                                        <td class="num s5">{lang.int(item.errors as u64)}</td>
                                        <td class="num">{lang.ms(item.avg_ms)}</td>
                                    </tr>
                                }
                            })
                            .collect_view()
                    }}
                </tbody>
            </table>
        </div>
    }
}
