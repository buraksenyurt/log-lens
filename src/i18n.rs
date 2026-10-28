//! A small, dependency-free translation layer.
//!
//! - [`Lang`] is the selected language. It lives in a signal shared through
//!   context ([`I18n`]), so every text that reads it updates when it changes.
//! - Fixed texts are fields of [`Texts`]; each language is one `const` value.
//!   A missing translation is a compile error (every field must be set).
//! - Texts with values in them, and number formats, are methods on [`Lang`]
//!   with an exhaustive `match`, so a new language cannot be forgotten either.

use leptos::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    De,
    Tr,
}

impl Lang {
    pub const ALL: [Lang; 3] = [Lang::En, Lang::De, Lang::Tr];

    /// BCP 47 code, used for `<html lang>`, `localStorage` and the selector.
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::De => "de",
            Lang::Tr => "tr",
        }
    }

    /// Name of the language in that language (shown in the selector).
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::De => "Deutsch",
            Lang::Tr => "Türkçe",
        }
    }

    /// Accepts codes such as `de`, `de-AT` or `TR`.
    pub fn from_code(code: &str) -> Option<Lang> {
        let primary = code.split(['-', '_']).next()?.to_ascii_lowercase();
        Lang::ALL.into_iter().find(|l| l.code() == primary)
    }

    pub fn texts(self) -> &'static Texts {
        match self {
            Lang::En => &EN,
            Lang::De => &DE,
            Lang::Tr => &TR,
        }
    }

    // ---------- number formats ----------

    fn decimal_separator(self) -> char {
        match self {
            Lang::En => '.',
            Lang::De | Lang::Tr => ',',
        }
    }

    fn group_separator(self) -> char {
        match self {
            Lang::En => ',',
            Lang::De | Lang::Tr => '.',
        }
    }

    /// 1234567 -> "1,234,567" (en) / "1.234.567" (de, tr)
    pub fn int(self, n: u64) -> String {
        let digits = n.to_string();
        let mut out = String::with_capacity(digits.len() + digits.len() / 3);
        for (i, c) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i) % 3 == 0 {
                out.push(self.group_separator());
            }
            out.push(c);
        }
        out
    }

    /// Fixed decimals with grouping: 1234.5 -> "1,234.5" (en) / "1.234,5" (de, tr)
    pub fn decimal(self, v: f64, decimals: usize) -> String {
        let s = format!("{:.*}", decimals, v.abs());
        let (int, frac) = s.split_once('.').unwrap_or((&s, ""));
        let mut out = String::new();
        if v.is_sign_negative() && v != 0.0 {
            out.push('-');
        }
        out.push_str(&self.int(int.parse().unwrap_or(0)));
        if !frac.is_empty() {
            out.push(self.decimal_separator());
            out.push_str(frac);
        }
        out
    }

    /// 13.04 -> "13.0%" (en) / "13,0 %" (de) / "%13,0" (tr)
    pub fn percent(self, v: f64) -> String {
        let n = self.decimal(v, 1);
        match self {
            Lang::En => format!("{n}%"),
            Lang::De => format!("{n}\u{a0}%"),
            Lang::Tr => format!("%{n}"),
        }
    }

    /// 1536 -> "1.5 KB" (en) / "1,5 KB" (de, tr)
    pub fn bytes(self, b: u64) -> String {
        const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
        let mut v = b as f64;
        let mut i = 0;
        while v >= 1024.0 && i < UNITS.len() - 1 {
            v /= 1024.0;
            i += 1;
        }
        if i == 0 {
            format!("{} B", self.int(b))
        } else {
            format!("{} {}", self.decimal(v, 1), UNITS[i])
        }
    }

    /// 1466.2 -> "1,466 ms" (en) / "1.466 ms" (de, tr)
    pub fn ms(self, v: f64) -> String {
        format!("{} ms", self.decimal(v, 0))
    }

    // ---------- texts with values ----------

    /// "access.log · 300 records · 0 invalid lines · parsed in 1.7 ms" (without the file name)
    pub fn file_summary(self, records: usize, failed: usize, parse_ms: f64) -> String {
        let (r, f, ms) = (self.int(records as u64), self.int(failed as u64), self.decimal(parse_ms, 1));
        match self {
            Lang::En => format!(" · {r} records · {f} invalid lines · parsed in {ms} ms"),
            Lang::De => format!(" · {r} Einträge · {f} ungültige Zeilen · in {ms} ms eingelesen"),
            Lang::Tr => format!(" · {r} kayıt · {f} hatalı satır · {ms} ms'de ayrıştırıldı"),
        }
    }

    pub fn parsing_progress(self, fraction: f64) -> String {
        let p = self.decimal(fraction * 100.0, 0);
        match self {
            Lang::En => format!("Parsing… {p}%"),
            Lang::De => format!("Wird eingelesen… {p}\u{a0}%"),
            Lang::Tr => format!("Ayrıştırılıyor… %{p}"),
        }
    }

    pub fn read_error(self, file: &str, reason: &str) -> String {
        match self {
            Lang::En => format!("Could not read {file}: {reason}"),
            Lang::De => format!("{file} konnte nicht gelesen werden: {reason}"),
            Lang::Tr => format!("{file} okunamadı: {reason}"),
        }
    }

    pub fn matches(self, n: usize) -> String {
        let n_fmt = self.int(n as u64);
        match self {
            Lang::En if n == 1 => "1 match".into(),
            Lang::En => format!("{n_fmt} matches"),
            Lang::De if n == 1 => "1 Treffer".into(),
            Lang::De => format!("{n_fmt} Treffer"),
            Lang::Tr => format!("{n_fmt} eşleşme"),
        }
    }

    pub fn page_of(self, page: usize, pages: usize) -> String {
        let (p, n) = (self.int(page as u64), self.int(pages as u64));
        match self {
            Lang::En => format!("Page {p} of {n}"),
            Lang::De => format!("Seite {p} von {n}"),
            Lang::Tr => format!("Sayfa {p} / {n}"),
        }
    }

    /// Tooltip of one histogram bar.
    pub fn hour_tooltip(self, hour: usize, requests: usize, errors: usize) -> String {
        let (r, e) = (self.int(requests as u64), self.int(errors as u64));
        match self {
            Lang::En => format!("{hour:02}:00 · {r} requests · {e} × 5xx"),
            Lang::De => format!("{hour:02}:00 · {r} Anfragen · {e} × 5xx"),
            Lang::Tr => format!("{hour:02}:00 · {r} istek · {e} adet 5xx"),
        }
    }
}

/// Every fixed text of the UI, for one language.
pub struct Texts {
    pub subtitle: &'static str,
    pub language: &'static str,
    pub drop_title: &'static str,
    pub drop_hint: &'static str,
    pub unknown_error: &'static str,
    // filter bar
    pub status: &'static str,
    pub method: &'static str,
    pub all: &'static str,
    pub path_contains: &'static str,
    pub min_duration: &'static str,
    pub endpoint: &'static str,
    pub remove_endpoint: &'static str,
    pub clear: &'static str,
    // stat cards
    pub requests: &'static str,
    pub error_rate: &'static str,
    pub unique_ips: &'static str,
    pub traffic: &'static str,
    pub avg_duration: &'static str,
    pub p95_duration: &'static str,
    // insights
    pub hourly: &'static str,
    pub top_endpoints: &'static str,
    pub path: &'static str,
    pub avg_short: &'static str,
    pub filter_by_endpoint: &'static str,
    // request table
    pub time: &'static str,
    pub size: &'static str,
    pub duration: &'static str,
    pub previous: &'static str,
    pub next: &'static str,
}

pub const EN: Texts = Texts {
    subtitle: "Analyse your Nginx logs in the browser. Your file is never sent to a server.",
    language: "Language",
    drop_title: "Drop a log file here",
    drop_hint: " or click to choose one",
    unknown_error: "unknown error",
    status: "Status",
    method: "Method",
    all: "All",
    path_contains: "Path contains",
    min_duration: "Min. duration (ms)",
    endpoint: "Endpoint",
    remove_endpoint: "Remove the endpoint filter",
    clear: "Clear",
    requests: "Requests",
    error_rate: "Error rate",
    unique_ips: "Unique IPs",
    traffic: "Traffic",
    avg_duration: "Avg. duration",
    p95_duration: "p95 duration",
    hourly: "Requests per hour",
    top_endpoints: "Busiest endpoints",
    path: "Path",
    avg_short: "Avg.",
    filter_by_endpoint: "Filter by this endpoint (click again to remove)",
    time: "Time",
    size: "Size",
    duration: "Duration",
    previous: "‹ Previous",
    next: "Next ›",
};

pub const DE: Texts = Texts {
    subtitle: "Analysiere deine Nginx-Logs im Browser. Deine Datei wird an keinen Server gesendet.",
    language: "Sprache",
    drop_title: "Logdatei hierher ziehen",
    drop_hint: " oder zum Auswählen klicken",
    unknown_error: "unbekannter Fehler",
    status: "Status",
    method: "Methode",
    all: "Alle",
    path_contains: "Pfad enthält",
    min_duration: "Min. Dauer (ms)",
    endpoint: "Endpunkt",
    remove_endpoint: "Endpunkt-Filter entfernen",
    clear: "Zurücksetzen",
    requests: "Anfragen",
    error_rate: "Fehlerquote",
    unique_ips: "Eindeutige IPs",
    traffic: "Datenvolumen",
    avg_duration: "Ø Dauer",
    p95_duration: "p95-Dauer",
    hourly: "Anfragen pro Stunde",
    top_endpoints: "Meistgenutzte Endpunkte",
    path: "Pfad",
    avg_short: "Ø",
    filter_by_endpoint: "Nach diesem Endpunkt filtern (erneut klicken zum Entfernen)",
    time: "Zeit",
    size: "Größe",
    duration: "Dauer",
    previous: "‹ Zurück",
    next: "Weiter ›",
};

pub const TR: Texts = Texts {
    subtitle: "Nginx log'larını tarayıcında analiz et. Dosyan hiçbir sunucuya gönderilmez.",
    language: "Dil",
    drop_title: "Log dosyasını buraya sürükle",
    drop_hint: " ya da tıklayıp seç",
    unknown_error: "bilinmeyen hata",
    status: "Status",
    method: "Method",
    all: "Tümü",
    path_contains: "Path içerir",
    min_duration: "Min. süre (ms)",
    endpoint: "Endpoint",
    remove_endpoint: "Endpoint filtresini kaldır",
    clear: "Temizle",
    requests: "İstek",
    error_rate: "Hata oranı",
    unique_ips: "Tekil IP",
    traffic: "Trafik",
    avg_duration: "Ort. süre",
    p95_duration: "p95 süre",
    hourly: "Saatlik dağılım",
    top_endpoints: "En yoğun endpoint'ler",
    path: "Path",
    avg_short: "Ort.",
    filter_by_endpoint: "Bu endpoint'e göre filtrele (tekrar tıklayınca kaldırılır)",
    time: "Zaman",
    size: "Boyut",
    duration: "Süre",
    previous: "‹ Önceki",
    next: "Sonraki ›",
};

const STORAGE_KEY: &str = "loglens.lang";

/// The selected language, shared through context. `Copy`, like a signal.
#[derive(Clone, Copy)]
pub struct I18n {
    pub lang: RwSignal<Lang>,
}

impl I18n {
    /// Starts with the language saved in `localStorage`, else the browser's
    /// language, else English. Saves every change and keeps `<html lang>` in sync.
    pub fn new() -> Self {
        let lang = RwSignal::new(initial_lang());
        Effect::new(move |_| {
            let lang = lang.get();
            save_lang(lang);
            if let Some(html) = document().document_element() {
                let _ = html.set_attribute("lang", lang.code());
            }
        });
        Self { lang }
    }

    /// Current language (tracked: callers re-run when it changes).
    pub fn lang(&self) -> Lang {
        self.lang.get()
    }

    /// Fixed texts of the current language (tracked).
    pub fn t(&self) -> &'static Texts {
        self.lang.get().texts()
    }
}

/// Shortcut for components: `let i18n = use_i18n();`
pub fn use_i18n() -> I18n {
    expect_context::<I18n>()
}

fn initial_lang() -> Lang {
    let saved = window()
        .local_storage()
        .ok()
        .flatten()
        .and_then(|s| s.get_item(STORAGE_KEY).ok().flatten());
    let browser = window().navigator().language();
    saved
        .as_deref()
        .and_then(Lang::from_code)
        .or_else(|| browser.as_deref().and_then(Lang::from_code))
        .unwrap_or(Lang::En)
}

fn save_lang(lang: Lang) {
    // Storage can be unavailable (private mode, blocked site data): ignore.
    if let Ok(Some(storage)) = window().local_storage() {
        let _ = storage.set_item(STORAGE_KEY, lang.code());
    }
}

/// Language selector for the page header.
#[component]
pub fn LanguageSelect() -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <label class="lang-select">
            <span class="visually-hidden">{move || i18n.t().language}</span>
            <select
                aria-label=move || i18n.t().language
                on:change=move |ev| {
                    if let Some(lang) = Lang::from_code(&event_target_value(&ev)) {
                        i18n.lang.set(lang);
                    }
                }
            >
                {Lang::ALL
                    .into_iter()
                    .map(|l| {
                        view! {
                            <option value=l.code() selected=move || i18n.lang() == l>
                                {l.native_name()}
                            </option>
                        }
                    })
                    .collect_view()}
            </select>
        </label>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_codes() {
        assert_eq!(Lang::from_code("de-AT"), Some(Lang::De));
        assert_eq!(Lang::from_code("TR"), Some(Lang::Tr));
        assert_eq!(Lang::from_code("en_US"), Some(Lang::En));
        assert_eq!(Lang::from_code("fr"), None);
        for l in Lang::ALL {
            assert_eq!(Lang::from_code(l.code()), Some(l));
        }
    }

    #[test]
    fn number_formats() {
        assert_eq!(Lang::En.int(1234567), "1,234,567");
        assert_eq!(Lang::Tr.int(1234567), "1.234.567");
        assert_eq!(Lang::De.int(999), "999");
        assert_eq!(Lang::En.decimal(1234.56, 1), "1,234.6");
        assert_eq!(Lang::De.decimal(1234.56, 1), "1.234,6");
        assert_eq!(Lang::Tr.decimal(-0.25, 2), "-0,25");
        assert_eq!(Lang::En.percent(13.04), "13.0%");
        assert_eq!(Lang::De.percent(13.04), "13,0\u{a0}%");
        assert_eq!(Lang::Tr.percent(13.04), "%13,0");
        assert_eq!(Lang::En.bytes(512), "512 B");
        assert_eq!(Lang::Tr.bytes(6_606_028), "6,3 MB");
        assert_eq!(Lang::En.ms(1466.2), "1,466 ms");
        assert_eq!(Lang::Tr.ms(1466.2), "1.466 ms");
    }

    #[test]
    fn texts_with_values() {
        assert_eq!(Lang::En.matches(1), "1 match");
        assert_eq!(Lang::En.matches(2500), "2,500 matches");
        assert_eq!(Lang::Tr.matches(2500), "2.500 eşleşme");
        assert_eq!(Lang::De.page_of(2, 10), "Seite 2 von 10");
    }
}
