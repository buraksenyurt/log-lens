use crate::filter::Filters;
use crate::i18n::use_i18n;
use leptos::prelude::*;

#[component]
pub fn FilterBar(#[prop(into)] matched: Signal<usize>) -> impl IntoView {
    let f = expect_context::<Filters>();
    let i18n = use_i18n();
    let t = move || i18n.t();
    view! {
        <section class="filters">
            <label>
                {move || t().status}
                <select bind:value=f.status>
                    <option value="">{move || t().all}</option>
                    <option value="2">"2xx"</option>
                    <option value="3">"3xx"</option>
                    <option value="4">"4xx"</option>
                    <option value="5">"5xx"</option>
                </select>
            </label>
            <label>
                {move || t().method}
                <select bind:value=f.method>
                    <option value="">{move || t().all}</option>
                    <option>"GET"</option>
                    <option>"POST"</option>
                    <option>"PUT"</option>
                    <option>"DELETE"</option>
                </select>
            </label>
            <label class="grow">
                {move || t().path_contains}
                <input type="search" placeholder="/api/..." bind:value=f.path />
            </label>
            <label>
                {move || t().min_duration}
                <input type="number" min="0" step="50" placeholder="0" bind:value=f.min_ms />
            </label>
            {move || {
                f.endpoint
                    .get()
                    .map(|endpoint| {
                        view! {
                            <span class="chip">
                                {move || t().endpoint} ": " <code>{endpoint}</code>
                                <button
                                    title=move || t().remove_endpoint
                                    on:click=move |_| f.endpoint.set(None)
                                >
                                    "×"
                                </button>
                            </span>
                        }
                    })
            }}
            <button on:click=move |_| f.reset()>{move || t().clear}</button>
            <span class="matched">{move || i18n.lang().matches(matched.get())}</span>
        </section>
    }
}
