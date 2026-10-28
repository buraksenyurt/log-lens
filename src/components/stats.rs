use crate::i18n::use_i18n;
use crate::stats::Summary;
use leptos::prelude::*;

/// A single statistic card.
#[component]
fn StatCard(
    #[prop(into)] label: Signal<&'static str>,
    #[prop(into)] value: Signal<String>,
    #[prop(optional)] tone: &'static str,
) -> impl IntoView {
    view! {
        <div class=format!("card {tone}")>
            <span class="label">{label}</span>
            <span class="value">{value}</span>
        </div>
    }
}

/// The panel displaying all statistics.
#[component]
pub fn StatsPanel(#[prop(into)] summary: Signal<Summary>) -> impl IntoView {
    let i18n = use_i18n();
    let t = move || i18n.t();
    let lang = move || i18n.lang();
    let ms = move |v: Option<f64>| v.map(|v| lang().ms(v)).unwrap_or_else(|| "—".into());
    view! {
        <section class="cards">
            <StatCard
                label=Signal::derive(move || t().requests)
                value=move || lang().int(summary.read().total as u64)
            />
            <StatCard
                label=Signal::derive(move || t().error_rate)
                tone="bad"
                value=move || lang().percent(summary.read().error_rate())
            />
            <StatCard
                label=Signal::derive(|| "5xx")
                tone="bad"
                value=move || lang().int(summary.read().server_errors as u64)
            />
            <StatCard
                label=Signal::derive(move || t().unique_ips)
                value=move || lang().int(summary.read().unique_ips as u64)
            />
            <StatCard
                label=Signal::derive(move || t().traffic)
                value=move || lang().bytes(summary.read().total_bytes)
            />
            <StatCard
                label=Signal::derive(move || t().avg_duration)
                value=move || ms(summary.read().avg_ms)
            />
            <StatCard
                label=Signal::derive(move || t().p95_duration)
                tone="warn"
                value=move || ms(summary.read().p95_ms)
            />
        </section>
    }
}
