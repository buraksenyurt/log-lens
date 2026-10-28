use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{DragEvent, File, HtmlInputElement};

use crate::i18n::use_i18n;

/// Select file and drag-and-drop area.
/// The selected file is handed to `on_file`; reading it is up to the caller
/// (see `loader::load_file`, which parses the file while it streams in).
#[component]
pub fn FileDrop(on_file: Callback<File>) -> impl IntoView {
    let i18n = use_i18n();
    let (dragging, set_dragging) = signal(false);
    let read_file = move |file: File| on_file.run(file);

    let on_change = move |ev: leptos::ev::Event| {
        let input: HtmlInputElement = ev.target().unwrap().unchecked_into();

        if let Some(file) = input.files().and_then(|list| list.get(0)) {
            read_file(file);
        }
    };

    let on_drop = move |ev: DragEvent| {
        ev.prevent_default();
        set_dragging.set(false);

        if let Some(file) = ev
            .data_transfer()
            .and_then(|dt| dt.files())
            .and_then(|list| list.get(0))
        {
            read_file(file);
        }
    };

    view! {
        <label
            class="drop-zone"
            class:dragging=dragging
            on:dragover=move |ev: DragEvent| {
                ev.prevent_default();
                set_dragging.set(true);
            }
            on:dragleave=move |_| set_dragging.set(false)
            on:drop=on_drop
        >
            <input type="file" accept=".log,.txt" on:change=on_change />
            <strong>{move || i18n.t().drop_title}</strong>
            <span>{move || i18n.t().drop_hint}</span>
        </label>

    }
}
