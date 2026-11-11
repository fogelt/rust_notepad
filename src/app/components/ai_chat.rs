use crate::app::NotepadApp;
use crate::app::components::provider::Provider;
use crate::app::local_stt::{self, LocalSession};
use crate::app::models::{DictationEvent, DictationSource, DictationStream};
use crate::app::provider_stt::{self, ProviderSession};
use crate::app::theme::{
    ACCENT, ACCENT_TEXT, ASSISTANT_DEFAULT_WIDTH, ASSISTANT_MIN_WIDTH, BG_ELEVATED, BORDER,
    MIN_EDITOR_WIDTH, MIN_SIDE_PANEL_WIDTH, TEXT, panel_frame, panel_header,
};
use eframe::egui;
use egui::text::CCursorRange;
use egui::text_edit::TextEditState;
use egui_phosphor::regular as icons;
use std::sync::mpsc;

/// Build the prompt sent to the assistant: user instruction + current document.
fn build_prompt(instruction: &str, document: &str) -> String {
    format!(
        "You are helping edit a Markdown document in a notepad app.\n\
         Follow the user's instruction below. Return ONLY the resulting Markdown \
         (no code fences, no commentary) unless the user explicitly asks for an explanation.\n\n\
         --- USER INSTRUCTION ---\n\
         {instruction}\n\n\
         --- CURRENT DOCUMENT ---\n\
         {document}"
    )
}

/// Prompt for the optional assistant-designed HTML preview. The model is asked
/// to analyze the document and build a visual report, not just reformat it.
fn build_html_design_prompt(document: &str) -> String {
    let prompt = r#"You are a visual document designer and analyst. Read the Markdown below, understand what kind of document it is (for example an earnings report, proposal, meeting notes, README, article, or plan), and design a polished, self-contained HTML page that helps a reader understand it at a glance.

REQUIREMENTS
- Do NOT just reformat or copy the Markdown. Extract the key message, metrics, trends, comparisons, risks, actions, and notable facts, then present them in a designed layout.
- Use a hero section with a one-paragraph executive summary, KPI/stat cards, horizontal bar charts, progress bars, a pie or donut chart, vertical column charts, timelines, callout boxes, comparison tables, and badges where they genuinely help. Pick 4-7 components; do not cram in everything.
- Only visualize data that is actually present in the source. Never invent or estimate numbers. For qualitative documents, use structured callouts and summaries instead of fake charts.
- Keep all key facts and the document's meaning. You may reorganize and summarize, but do not omit important details.
- Return ONLY the complete HTML document, starting with <!DOCTYPE html>. No code fences, no commentary.
- Use one inline <style> block. No external stylesheets, fonts, or scripts. The only external resources allowed are the Commons images described below.
- Include one relevant hero photo near the top, and optionally a second one in a section. Use placeholder URLs of the form `commons:<2-4 descriptive words>`, for example `<img src='commons:finance office' alt='Finance office'>`. The app resolves these to public-domain Wikimedia Commons photos, wraps them in a full-width photo block, and adds a credit line. Do not use any other image URL, and never rely on an image to convey data.
- Do not use JavaScript. Charts must be pure HTML/CSS.
- Dark theme: page background #1c1d20, body text #e4e4e7, headings #ffffff, accent and links #84aee0, muted text #9a9ca3, dividers #3a3c44, raised cards #232428, positive #7ecb8f, warning #e0b878, negative #e08a8a.

RENDERER RULES (important)
The page is rendered by a lightweight engine, not a full browser. Follow these rules exactly:
- Do NOT use CSS grid, flexbox, gap, box-shadow, transform, clamp(), calc(), aspect-ratio, inset, vw/vh units, position:fixed, or position:sticky. Do not set min-width on tables, and do not use white-space:nowrap on long text.
- The only allowed positioning is inside pie/donut markup: position:relative on the chart wrapper, position:absolute for the donut hole and center label.
- Build multi-column layouts and card rows with <table> elements (width="100%", table-layout:fixed, one <td> per column). Put each card's background, padding, and border-radius directly on its <td>. Do not wrap cards in inner divs; those do not stretch to the row height.
- Use cell padding and margins for spacing, never gap.
- Use px font sizes: body 14-15px, h1 26-28px, h2 18-20px, h3 15-16px, small labels 11-12px. Keep line-height around 1.5.
- Horizontal bars: a 2-cell table row for the label and value, then a rounded track div with a child div whose width is the percentage (recipe below).
- Vertical columns: a table row of cells with vertical-align:bottom, each containing a fixed-height div for the bar and a label underneath.
- Timelines: a table with a narrow date cell and a content cell, or stacked rows with a colored left border.

RESPONSIVE RULES (mandatory)
The preview panel can be as narrow as 320px, so every report MUST be readable there with no horizontal overflow.
- Give every layout table (KPI row, card row, action row, image+text row) the class "stack". Give every chart+legend row the class "chart-row" and its two cells the class "chart-cell".
- Use at most 4 columns in data tables, never table-layout:fixed on them, and put class="hide-narrow" on columns that can disappear on phones.
- Avoid long unbroken words and tables with 5 or more columns.
- Include this block at the very end of <style>, unchanged:
@media (max-width: 640px) {
  body { padding: 14px 12px; font-size: 14px; }
  h1 { font-size: 22px; }
  h2 { font-size: 17px; margin: 18px 0 10px; }
  h3 { font-size: 15px; }
  .stack { border-spacing: 0; margin: 0; }
  .stack td { display: block; width: 100%; margin-bottom: 8px; }
  .chart-cell { display: block; width: 100%; }
  .hide-narrow { display: none; }
  .data { font-size: 12px; }
  .data th, .data td { padding: 6px 4px; }
}
- Tables with class "stack" must use border-collapse:separate and horizontal border-spacing so cards have gaps on desktop (for example border-spacing:9px 0; margin:0 -9px). The media query removes the spacing when cells stack.

PIE AND DONUT RECIPE (supported by the engine)
Use conic-gradient on a fixed-size square div, with a legend table beside it:
<table class="chart-row" width="100%"><tr>
<td class="chart-cell" width="55%" style="text-align:center;vertical-align:middle;">
  <div style="width:150px;height:150px;border-radius:50%;background:conic-gradient(#84aee0 0 52%, #7fc99a 52% 83%, #e0b878 83% 95%, #8a8f9c 95% 100%);margin:0 auto;"></div>
</td>
<td class="chart-cell" width="45%" style="vertical-align:middle;padding-left:14px;">
  <table width="100%"><tr>
    <td style="font-size:13px;padding:3px 0;"><span style="display:inline-block;width:11px;height:11px;border-radius:3px;background:#84aee0;"></span> North America</td>
    <td style="text-align:right;color:#9a9ca3;font-size:13px;">52%</td>
  </tr></table>
</td></tr></table>
- The conic-gradient stops must run from 0 to 100% in order and use the exact shares from the source data.
- For a donut, wrap the circle in <div style="position:relative;width:150px;height:150px;margin:0 auto;">, keep the circle div inside, and add <div style="position:absolute;left:25%;top:25%;width:50%;height:50%;border-radius:50%;background:#1c1d20;"></div> for the hole. Optionally add a centered label with <div style="position:absolute;left:0;top:60px;width:150px;text-align:center;font-size:20px;font-weight:700;color:#ffffff;">52%</div>.
- Never use aspect-ratio, ::after, or percentages for the chart size.

HORIZONTAL BAR RECIPE
<div style="margin-bottom:10px;">
  <table width="100%"><tr>
    <td style="font-weight:600;font-size:13px;">Online store</td>
    <td style="text-align:right;color:#9a9ca3;font-size:13px;">$2.6M &middot; 54%</td>
  </tr></table>
  <div style="width:100%;height:14px;background:#2b2d32;border-radius:7px;"><div style="width:54%;height:14px;border-radius:7px;background:#84aee0;"></div></div>
</div>

VISUAL STYLE
- Prefer background fills, spacing, and soft rounded corners over outlines. Do not put a 1px border around cards, panels, or sections; reserve borders for table rules and deliberate dividers.
- Keep the page focused and compact: aim for roughly 600-1000 words. Avoid repeating the source verbatim or adding sections that do not add insight.
- Design for 320px as you write: card labels should wrap in words, values stay large, and no element may be wider than its container.

--- MARKDOWN ---
"#;
    format!("{prompt}{document}")
}

/// Strip a wrapping ```markdown fence if the model added one anyway.
fn strip_code_fence(text: &str) -> String {
    let trimmed = text.trim();
    if let Some(inner) = trimmed
        .strip_prefix("```")
        .and_then(|s| s.strip_suffix("```"))
    {
        // Drop an optional language tag on the first line (`markdown`, `md`).
        let mut lines = inner.lines();
        let first = lines.next().unwrap_or("");
        let first_trim = first.trim();
        if first_trim.eq_ignore_ascii_case("markdown") || first_trim.eq_ignore_ascii_case("md") {
            return lines.collect::<Vec<_>>().join("\n").trim().to_string();
        }
        return inner.trim().to_string();
    }
    trimmed.to_string()
}

/// Pull the first complete HTML document out of a model response, ignoring
/// any commentary or markdown fence around it.
fn extract_html_document(raw: &str) -> Result<String, String> {
    let lower = raw.to_ascii_lowercase();
    let start = lower
        .find("<!doctype html")
        .or_else(|| lower.find("<html"))
        .ok_or_else(|| {
            String::from("The assistant returned an explanation instead of an HTML page.")
        })?;
    let end = lower.rfind("</html>").ok_or_else(|| {
        String::from("The visual report came back incomplete. Try again, or choose a faster model.")
    })? + "</html>".len();
    if end <= start {
        return Err(String::from(
            "The visual report came back incomplete. Try again, or choose a faster model.",
        ));
    }
    Ok(raw[start..end].to_string())
}

struct CommonsImage {
    thumb_url: String,
    description_url: String,
    title: String,
    license: String,
    width: u32,
    height: u32,
}

/// Replace `src="commons:<keywords>"` placeholders with public-domain
/// Wikimedia Commons photos and add a small credit line under each.
fn resolve_commons_images(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let mut out = String::with_capacity(html.len());
    let mut cursor = 0;
    let mut resolved = 0;
    while let Some(relative) = lower[cursor..].find("<img") {
        let start = cursor + relative;
        out.push_str(&html[cursor..start]);
        let Some(rel_end) = lower[start..].find('>') else {
            out.push_str(&html[start..]);
            return out;
        };
        let end = start + rel_end + 1;
        let tag = &html[start..end];
        if resolved < 2
            && let Some(keywords) = tag_commons_keywords(tag)
            && let Some(image) = commons_image(&keywords)
        {
            let alt = image.title.strip_prefix("File:").unwrap_or(&image.title);
            out.push_str(&image_figure(&image, alt));
            resolved += 1;
        }
        // Images we couldn't resolve are dropped so no broken icon is shown.
        cursor = end;
    }
    out.push_str(&html[cursor..]);
    out
}

fn tag_commons_keywords(tag: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let value_start = lower.find("src=")? + 4;
    let rest = tag[value_start..].trim_start();
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let inner = &rest[1..];
    let end = inner.find(quote)?;
    let value = &inner[..end];
    value
        .strip_prefix("commons:")
        .map(|keywords| keywords.trim().to_string())
        .filter(|keywords| !keywords.is_empty())
}

fn credit_html(image: &CommonsImage) -> String {
    let title = escape_html(image.title.strip_prefix("File:").unwrap_or(&image.title));
    format!(
        "<div style=\"font-size:11px;color:#9a9ca4;margin:4px 0 0;\">Photo: \
         <a href=\"{}\" style=\"color:#84aee0;\">{}</a> &middot; {}</div>",
        escape_html(&image.description_url),
        title,
        escape_html(&image.license)
    )
}

/// Render a photo as a padded background-image box.
///
/// litehtml draws `<img>` at its intrinsic pixel size and clips it to the CSS
/// box, so a `width:100%` photo only fills the preview at one width. A
/// background box with a percentage `padding-top` keeps the aspect ratio and
/// always fills the available width, from a narrow panel to a wide window.
fn image_figure(image: &CommonsImage, alt: &str) -> String {
    let ratio = if image.width > 0 && image.height > 0 {
        (image.height as f32 / image.width as f32).clamp(0.5, 1.4)
    } else {
        0.75
    };
    format!(
        "<div style=\"margin:0 0 18px 0;\">\
         <div role=\"img\" aria-label=\"{alt}\" \
         style=\"width:100%;padding-top:{padding:.2}%;background-image:url('{url}');\
         background-size:cover;background-position:center;border-radius:10px;\"></div>\
         {credit}</div>",
        alt = escape_html(alt),
        padding = ratio * 100.0,
        url = escape_html(&image.thumb_url),
        credit = credit_html(image),
    )
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn commons_image(keywords: &str) -> Option<CommonsImage> {
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .ok()?;
    let search = format!("filetype:bitmap {keywords}");
    let resp = client
        .get("https://commons.wikimedia.org/w/api.php")
        .query(&[
            ("action", "query"),
            ("generator", "search"),
            ("gsrsearch", search.as_str()),
            ("gsrnamespace", "6"),
            ("gsrlimit", "8"),
            ("prop", "imageinfo"),
            ("iiprop", "url|extmetadata"),
            ("iiurlwidth", "960"),
            ("format", "json"),
            ("utf8", "1"),
        ])
        .header(
            "User-Agent",
            "RustyNotepad/0.3 (https://github.com/fogelt/rust_notepad)",
        )
        .send()
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let json: serde_json::Value = resp.json().ok()?;
    let pages = json["query"]["pages"].as_object()?;
    for page in pages.values() {
        let info = &page["imageinfo"][0];
        let license = info["extmetadata"]["LicenseShortName"]["value"]
            .as_str()
            .unwrap_or("");
        let license_lower = license.to_ascii_lowercase();
        if !(license_lower.contains("cc0")
            || license_lower.contains("public domain")
            || license_lower == "pd")
        {
            continue;
        }
        let Some(thumb_url) = info["thumburl"].as_str() else {
            continue;
        };
        let description_url = info["descriptionurl"].as_str().unwrap_or("");
        let title = page["title"].as_str().unwrap_or("");
        if thumb_url.is_empty() || description_url.is_empty() || title.is_empty() {
            continue;
        }
        return Some(CommonsImage {
            thumb_url: thumb_url.to_string(),
            description_url: description_url.to_string(),
            title: title.to_string(),
            license: license.to_string(),
            width: info["thumbwidth"].as_u64().unwrap_or(0) as u32,
            height: info["thumbheight"].as_u64().unwrap_or(0) as u32,
        });
    }
    None
}

/// If the model didn't include any usable image, add a hero photo chosen from
/// the document's first heading so every visual report has one.
fn inject_fallback_image(html: &str, document: &str) -> String {
    let lower = html.to_ascii_lowercase();
    if lower.contains("<img") || lower.contains("background-image:url(") {
        return html.to_string();
    }
    let queries = document_image_queries(document);
    // Prefer a photo whose title matches the topic word (the trailing word of
    // the heading). A brand-led heading like "Solvik Coffee" can otherwise
    // resolve to a photo of the place named Solvik. Keep the first result as a
    // last resort when nothing matches.
    let topic = queries.get(1).cloned();
    let mut first_result = None;
    let mut chosen = None;
    for query in &queries {
        let Some(image) = commons_image(query) else {
            continue;
        };
        let matches_topic = topic
            .as_ref()
            .is_none_or(|topic| image.title.to_ascii_lowercase().contains(topic));
        if matches_topic {
            chosen = Some(image);
            break;
        }
        if first_result.is_none() {
            first_result = Some(image);
        }
    }
    let Some(image) = chosen.or(first_result) else {
        return html.to_string();
    };
    let alt = image.title.strip_prefix("File:").unwrap_or(&image.title);
    let figure = image_figure(&image, alt);

    let lower = html.to_ascii_lowercase();
    if let Some(pos) = lower.find("</h1>") {
        let at = pos + "</h1>".len();
        return format!("{}{}{}", &html[..at], figure, &html[at..]);
    }
    if let Some(body) = lower.find("<body")
        && let Some(open) = lower[body..].find('>')
    {
        let at = body + open + 1;
        return format!("{}{}{}", &html[..at], figure, &html[at..]);
    }
    format!("{figure}{html}")
}

/// Queries to try for a fallback image, based on the document's first heading.
fn document_image_queries(document: &str) -> Vec<String> {
    const STOPWORDS: &[&str] = &[
        "the",
        "a",
        "an",
        "and",
        "or",
        "for",
        "of",
        "to",
        "in",
        "on",
        "with",
        "report",
        "review",
        "quarter",
        "quarterly",
        "summary",
        "overview",
        "notes",
        "results",
        "update",
    ];
    let Some(line) = document
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
    else {
        return Vec::new();
    };
    let text = line.trim_start_matches('#').trim();
    let words: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| {
            word.len() > 2
                && !word.chars().all(|c| c.is_ascii_digit())
                && !STOPWORDS.contains(&word.to_ascii_lowercase().as_str())
        })
        .map(str::to_ascii_lowercase)
        .collect();
    let mut queries = Vec::new();
    if !words.is_empty() {
        queries.push(words.iter().take(3).cloned().collect::<Vec<_>>().join(" "));
        // Prefer trailing words for the single-word fallbacks: headings often
        // lead with a brand ("Solvik Coffee"), and the trailing topic word
        // gives a far more relevant photo than the brand name.
        for word in words.iter().rev().take(2) {
            if !queries.contains(word) {
                queries.push(word.clone());
            }
        }
    }
    queries
}

/// Marker identifying Rusty Notepad's styled HTML sidecar. An unrelated
/// `document.html` next to the Markdown is never loaded as a preview.
const STYLED_HTML_MARKER: &str = "<!-- rusty-notepad-styled-html -->";

/// Read an attribute value from an opening tag (case-insensitive name).
fn tag_attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let lower = tag.to_ascii_lowercase();
    let mut cursor = 0;
    while let Some(relative) = lower[cursor..].find(name) {
        let start = cursor + relative;
        let before_ok = start == 0
            || !lower.as_bytes()[start - 1].is_ascii_alphanumeric()
                && lower.as_bytes()[start - 1] != b'-';
        let mut end = start + name.len();
        while lower.as_bytes().get(end) == Some(&b' ') {
            end += 1;
        }
        if before_ok && lower.as_bytes().get(end) == Some(&b'=') {
            end += 1;
            while lower.as_bytes().get(end) == Some(&b' ') {
                end += 1;
            }
            let quote = *lower.as_bytes().get(end)?;
            if quote == b'"' || quote == b'\'' {
                let inner_start = end + 1;
                let inner_end = tag[inner_start..].find(quote as char)? + inner_start;
                return Some(&tag[inner_start..inner_end]);
            }
        }
        cursor = start + name.len();
    }
    None
}

/// Append a class name to a table's `class` attribute, adding one if needed.
fn add_tag_class(tag: &str, class_name: &str) -> String {
    if let Some(existing) = tag_attribute(tag, "class") {
        let lower = tag.to_ascii_lowercase();
        let Some(pos) = lower.find(existing) else {
            return tag.to_string();
        };
        let mut out = String::with_capacity(tag.len() + class_name.len() + 1);
        out.push_str(&tag[..pos]);
        out.push_str(existing);
        out.push(' ');
        out.push_str(class_name);
        out.push_str(&tag[pos + existing.len()..]);
        return out;
    }
    let Some(pos) = tag.rfind('>') else {
        return tag.to_string();
    };
    format!("{} class=\"{}\"{}", &tag[..pos], class_name, &tag[pos..])
}

/// Give card/layout tables a `stack` class so the narrow-screen rules can turn
/// their cells into full-width rows. Data tables (with `<th>` header cells) are
/// left alone.
pub(crate) fn mark_layout_tables(html: &str) -> String {
    const CARD_HINTS: &[&str] = &[
        "kpi",
        "card",
        "stat",
        "tile",
        "metric",
        "action",
        "feature",
        "grid",
        "two-col",
        "three-col",
        "cols",
        "split",
        "compare",
        "steps",
        "timeline",
        "panel",
    ];
    let lower = html.to_ascii_lowercase();
    let mut out = String::with_capacity(html.len());
    let mut cursor = 0;

    while let Some(relative) = lower[cursor..].find("<table") {
        let start = cursor + relative;
        out.push_str(&html[cursor..start]);
        let Some(close) = lower[start..].find('>') else {
            out.push_str(&html[start..]);
            return out;
        };
        let tag_end = start + close + 1;
        let tag = &html[start..tag_end];
        let window_end = (tag_end + 4000).min(lower.len());
        let row_scope = &lower[tag_end..window_end];
        let row = match row_scope.find("</tr>") {
            Some(end) => &row_scope[..end],
            None => row_scope,
        };
        let has_header = row.contains("<th");
        let cells = row.matches("<td").count();
        let class_lower = tag_attribute(tag, "class")
            .unwrap_or_default()
            .to_ascii_lowercase();
        let style_lower = tag_attribute(tag, "style")
            .unwrap_or_default()
            .to_ascii_lowercase();
        let card_like = CARD_HINTS.iter().any(|hint| class_lower.contains(hint))
            || style_lower.contains("background")
            || row.contains("background");
        let already_stacked = class_lower.split_whitespace().any(|name| name == "stack");

        if !has_header && cells >= 2 && card_like && !already_stacked {
            out.push_str(&add_tag_class(tag, "stack"));
        } else {
            out.push_str(tag);
        }
        cursor = tag_end;
    }
    out.push_str(&html[cursor..]);
    out
}

/// Safety net for reports that forgot the narrow-screen rules: inject a small
/// responsive block so the page stays readable in a narrow preview.
pub(crate) fn ensure_narrow_screen_support(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    if lower.contains(".stack td") {
        return html.to_string();
    }
    const RULES: &str = "@media (max-width: 640px) {\n\
        body { padding: 14px 12px; }\n\
        h1 { font-size: 22px; }\n\
        h2 { font-size: 17px; }\n\
        h3 { font-size: 15px; }\n\
        table { table-layout: auto !important; }\n\
        .stack { border-spacing: 0 !important; margin: 0 !important; }\n\
        .stack td { display: block !important; width: 100% !important; }\n\
        .chart-cell { display: block !important; width: 100% !important; }\n\
        .hide-narrow { display: none !important; }\n\
        }\n";

    if let Some(pos) = lower.rfind("</style>") {
        return format!("{}{}{}", &html[..pos], RULES, &html[pos..]);
    }
    if let Some(pos) = lower.rfind("</head>") {
        return format!("{}<style>{RULES}</style>{}", &html[..pos], &html[pos..]);
    }
    html.to_string()
}

fn ensure_styled_marker(html: &str) -> String {
    if html.contains(STYLED_HTML_MARKER) {
        return html.to_string();
    }
    let trimmed = html.trim_start();
    let insert_at = if trimmed.to_ascii_lowercase().starts_with("<!doctype html>") {
        html.len() - trimmed.len() + "<!doctype html>".len()
    } else {
        0
    };
    if insert_at == 0 {
        format!("{STYLED_HTML_MARKER}\n{html}")
    } else {
        let (head, tail) = html.split_at(insert_at);
        format!("{head}\n{STYLED_HTML_MARKER}{tail}")
    }
}

/// Generate text with the selected provider. `max_output_tokens` is a
/// best-effort cap: OpenAI's Responses API and Anthropic's Messages API both
/// accept it directly.
fn generate_text(
    provider: Provider,
    prompt: &str,
    api_key: &str,
    model: &str,
    max_output_tokens: u32,
    temperature: f32,
) -> Result<String, String> {
    let text = match provider {
        Provider::Gemini => {
            gemini_generate_with(prompt, api_key, model, max_output_tokens, temperature)?
        }
        Provider::OpenAi => openai_generate(prompt, api_key, model, max_output_tokens)?,
        Provider::Anthropic => anthropic_generate(prompt, api_key, model, max_output_tokens)?,
        Provider::OpenRouter => {
            openrouter_generate(prompt, api_key, model, max_output_tokens, temperature)?
        }
        Provider::DeepSeek => {
            deepseek_generate(prompt, api_key, model, max_output_tokens, temperature)?
        }
    };
    Ok(strip_code_fence(&text))
}

fn gemini_generate_with(
    prompt: &str,
    api_key: &str,
    model: &str,
    max_output_tokens: u32,
    temperature: f32,
) -> Result<String, String> {
    let body = serde_json::json!({
        "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
        "generationConfig": { "temperature": temperature, "maxOutputTokens": max_output_tokens }
    });
    let json = post_generate(&body, api_key, model)?;
    response_text(&json).ok_or_else(|| "Gemini returned an empty response.".to_string())
}

/// OpenAI's Responses API. Reasoning models reject a custom temperature, so it
/// is left at the model default here.
fn openai_generate(
    prompt: &str,
    api_key: &str,
    model: &str,
    max_output_tokens: u32,
) -> Result<String, String> {
    let body = serde_json::json!({
        "model": model.trim(),
        "input": prompt,
        "max_output_tokens": max_output_tokens,
        // Don't keep the document on OpenAI's servers after the reply.
        "store": false,
    });
    let resp = http_client()?
        .post("https://api.openai.com/v1/responses")
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .map_err(|e| format!("Request failed: {e}"))?;
    let json = check_response(resp, "OpenAI")?;
    openai_text(&json).ok_or_else(|| "OpenAI returned an empty response.".to_string())
}

/// Anthropic's Messages API. Temperature is omitted because the current
/// models use adaptive thinking, which rejects custom values.
fn anthropic_generate(
    prompt: &str,
    api_key: &str,
    model: &str,
    max_output_tokens: u32,
) -> Result<String, String> {
    let body = serde_json::json!({
        "model": model.trim(),
        "max_tokens": max_output_tokens,
        "messages": [{ "role": "user", "content": prompt }],
    });
    let resp = http_client()?
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .map_err(|e| format!("Request failed: {e}"))?;
    let json = check_response(resp, "Anthropic")?;
    anthropic_text(&json).ok_or_else(|| "Anthropic returned an empty response.".to_string())
}

/// OpenRouter exposes an OpenAI-compatible chat completions endpoint.
fn openrouter_generate(
    prompt: &str,
    api_key: &str,
    model: &str,
    max_output_tokens: u32,
    temperature: f32,
) -> Result<String, String> {
    let body = serde_json::json!({
        "model": model.trim(),
        "messages": [{ "role": "user", "content": prompt }],
        "max_tokens": max_output_tokens,
        "temperature": temperature,
    });
    let resp = http_client()?
        .post("https://openrouter.ai/api/v1/chat/completions")
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .map_err(|e| format!("Request failed: {e}"))?;
    let json = check_response(resp, "OpenRouter")?;
    chat_completion_text(&json).ok_or_else(|| "OpenRouter returned an empty response.".to_string())
}

/// DeepSeek is OpenAI-compatible. The reasoner model ignores temperature, so
/// it is only sent for the chat model.
fn deepseek_generate(
    prompt: &str,
    api_key: &str,
    model: &str,
    max_output_tokens: u32,
    temperature: f32,
) -> Result<String, String> {
    let mut body = serde_json::json!({
        "model": model.trim(),
        "messages": [{ "role": "user", "content": prompt }],
        "max_tokens": max_output_tokens,
    });
    if !model.contains("reasoner") {
        body["temperature"] = serde_json::json!(temperature);
    }
    let resp = http_client()?
        .post("https://api.deepseek.com/chat/completions")
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .map_err(|e| format!("Request failed: {e}"))?;
    let json = check_response(resp, "DeepSeek")?;
    chat_completion_text(&json).ok_or_else(|| "DeepSeek returned an empty response.".to_string())
}

fn http_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("Request failed: {e}"))
}

fn check_response(
    resp: reqwest::blocking::Response,
    provider: &str,
) -> Result<serde_json::Value, String> {
    if !resp.status().is_success() {
        let status = resp.status();
        let detail = resp.text().unwrap_or_default();
        let short = detail.chars().take(300).collect::<String>();
        return Err(format!("{provider} error {status}: {short}"));
    }
    resp.json()
        .map_err(|e| format!("Bad response from {provider}: {e}"))
}

/// Text from an OpenAI Responses payload: either the convenience
/// `output_text` field or the message items inside `output`.
fn openai_text(json: &serde_json::Value) -> Option<String> {
    if let Some(text) = json["output_text"].as_str()
        && !text.trim().is_empty()
    {
        return Some(text.to_string());
    }
    let mut out = String::new();
    for item in json["output"].as_array()? {
        if item["type"].as_str() != Some("message") {
            continue;
        }
        for part in item["content"].as_array()? {
            if part["type"].as_str() == Some("output_text")
                && let Some(text) = part["text"].as_str()
            {
                out.push_str(text);
            }
        }
    }
    (!out.trim().is_empty()).then_some(out)
}

/// Text from an OpenAI-compatible chat completions payload.
fn chat_completion_text(json: &serde_json::Value) -> Option<String> {
    let text = json["choices"][0]["message"]["content"].as_str()?;
    (!text.trim().is_empty()).then(|| text.to_string())
}

/// Text from an Anthropic Messages payload.
fn anthropic_text(json: &serde_json::Value) -> Option<String> {
    let mut out = String::new();
    for part in json["content"].as_array()? {
        if part["type"].as_str() == Some("text")
            && let Some(text) = part["text"].as_str()
        {
            out.push_str(text);
        }
    }
    (!out.trim().is_empty()).then_some(out)
}

/// Turn raw provider/network errors into something a person can act on.
fn friendly_generation_error(raw: &str, provider: Provider) -> String {
    friendly_error(raw, provider)
}

fn friendly_error(raw: &str, provider: Provider) -> String {
    let name = provider.label();
    let lower = raw.to_ascii_lowercase();
    if lower.contains("api key")
        || lower.contains("api_key")
        || lower.contains("unauthenticated")
        || lower.contains("permission")
        || lower.contains("invalid_api_key")
        || lower.contains("401")
        || lower.contains("403")
    {
        return format!("{name} rejected the API key. Check it in Settings > AI and try again.");
    }
    if lower.contains("429")
        || lower.contains("quota")
        || lower.contains("rate limit")
        || lower.contains("resource_exhausted")
    {
        return format!("{name} is rate limiting requests right now. Wait a moment and try again.");
    }
    if lower.contains("404") || lower.contains("not found") || lower.contains("not supported") {
        return format!(
            "That model isn't available with {name}. Pick another model in Settings > AI."
        );
    }
    if lower.contains("request failed")
        || lower.contains("sending request")
        || lower.contains("connection")
        || lower.contains("timed out")
        || lower.contains("dns")
        || lower.contains("network")
        || lower.contains("could not connect")
    {
        return format!("Could not reach {name}. Check your internet connection and try again.");
    }
    let short = raw.chars().take(220).collect::<String>();
    format!("Request failed: {short}")
}

/// Offline dictation errors are already written for people; just add a prefix
/// when the raw message is technical.
fn friendly_local_error(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    if lower.contains("microphone") || lower.contains("dictation") || lower.contains("model") {
        return raw.to_string();
    }
    let short = raw.chars().take(220).collect::<String>();
    format!("Local dictation failed: {short}")
}

/// Join the text parts of the first candidate, skipping any "thought" parts.
fn response_text(json: &serde_json::Value) -> Option<String> {
    let parts = json["candidates"][0]["content"]["parts"].as_array()?;
    let text: String = parts
        .iter()
        .filter(|part| part.get("thought").and_then(|v| v.as_bool()) != Some(true))
        .filter_map(|part| part["text"].as_str())
        .collect();
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

fn post_generate(
    body: &serde_json::Value,
    api_key: &str,
    model: &str,
) -> Result<serde_json::Value, String> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
        model.trim()
    );
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("Request failed: {e}"))?;
    let resp = client
        .post(&url)
        .query(&[("key", api_key)])
        .json(body)
        .send()
        .map_err(|e| format!("Request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let detail = resp.text().unwrap_or_default();
        let short = detail.chars().take(300).collect::<String>();
        return Err(format!("Gemini error {status}: {short}"));
    }

    resp.json()
        .map_err(|e| format!("Bad response from Gemini: {e}"))
}

/// Prepare an edit for streamed dictation: replace the selected character
/// range, add a leading space if it would run into the previous word, and
/// report where deltas should be inserted. The returned `bool` says whether a
/// trailing space is needed once the stream finishes.
fn prepare_dictation(content: &str, first: usize, second: usize) -> (String, usize, bool) {
    let (start_c, end_c) = if first <= second {
        (first, second)
    } else {
        (second, first)
    };
    let start_b = content
        .char_indices()
        .nth(start_c)
        .map(|(b, _)| b)
        .unwrap_or(content.len());
    let end_b = content
        .char_indices()
        .nth(end_c)
        .map(|(b, _)| b)
        .unwrap_or(content.len());

    let before = &content[..start_b];
    let after = &content[end_b..];
    let before_space = before
        .chars()
        .next_back()
        .is_some_and(|c| c.is_whitespace());
    let after_space = after.chars().next().is_some_and(|c| c.is_whitespace());

    let mut updated = String::with_capacity(content.len() + 1);
    updated.push_str(before);
    if !before.is_empty() && !before_space {
        updated.push(' ');
    }
    let insert_at = updated.len();
    updated.push_str(after);
    let needs_trailing = !after.is_empty() && !after_space;
    (updated, insert_at, needs_trailing)
}

fn insert_at_cursor(app: &mut NotepadApp, ctx: &egui::Context, snippet: &str) {
    let id = egui::Id::new("editor_text");
    if let Some(state) = TextEditState::load(ctx, id)
        && let Some(range) = state.cursor.char_range()
    {
        let idx = range.primary.index.max(range.secondary.index);
        let byte_pos = app
            .content
            .char_indices()
            .nth(idx)
            .map(|(b, _)| b)
            .unwrap_or(app.content.len());
        let mut to_insert = snippet.to_string();
        if !to_insert.ends_with('\n') {
            to_insert.push('\n');
        }
        app.content.insert_str(byte_pos, &to_insert);
        let new_c = app.content[..byte_pos + to_insert.len()].chars().count();
        if let Some(mut st) = TextEditState::load(ctx, id) {
            st.cursor
                .set_char_range(Some(CCursorRange::one(egui::text::CCursor::new(new_c))));
            st.store(ctx, id);
        }
        ctx.memory_mut(|m| m.request_focus(id));
        return;
    }
    if !app.content.is_empty() && !app.content.ends_with('\n') {
        app.content.push('\n');
    }
    app.content.push_str(snippet);
    if !app.content.ends_with('\n') {
        app.content.push('\n');
    }
}

impl NotepadApp {
    fn poll_chat(&mut self) {
        let result = match self.chat_rx.as_ref().map(|rx| rx.try_recv()) {
            Some(Ok(result)) => Some(result),
            Some(Err(mpsc::TryRecvError::Disconnected)) => Some(Err(String::from(
                "The request stopped unexpectedly. Please try again.",
            ))),
            _ => None,
        };
        let Some(result) = result else {
            return;
        };
        self.chat_rx = None;
        match result {
            Ok(text) => {
                self.chat_response = text;
                self.status_message = String::from("Suggestion ready.");
            }
            Err(err) => {
                // Server errors belong in a toast; the suggestion card is for
                // the model's output, not transport failures.
                self.status_message = String::from("The assistant request failed.");
                self.push_toast(friendly_generation_error(&err, self.chat_provider), true);
            }
        }
        self.chat_busy = false;
    }

    fn poll_html(&mut self) {
        let result = match self.html_rx.as_ref().map(|rx| rx.try_recv()) {
            Some(Ok(result)) => Some(result),
            Some(Err(mpsc::TryRecvError::Disconnected)) => Some(Err(String::from(
                "The request stopped unexpectedly. Please try again.",
            ))),
            _ => None,
        };
        let Some(result) = result else {
            return;
        };
        self.html_rx = None;
        self.html_busy = false;
        self.html_started = None;
        match result {
            Ok(html) => self.apply_styled_html(html),
            Err(err) => {
                let message = friendly_generation_error(&err, self.chat_provider);
                self.status_message = String::from("Could not create the visual report.");
                self.html_error = Some(message.clone());
                self.push_toast(message, true);
            }
        }
    }

    /// Poll recording and transcription for document dictation. Called once
    /// per frame so it also runs while the Assistant panel is closed.
    pub fn poll_dictation(&mut self, ctx: &egui::Context) {
        self.poll_local_dictation(ctx);
        self.poll_provider_dictation(ctx);
        self.poll_whisper_download(ctx);
        self.reveal_dictation(ctx);
        if self.is_dictating() {
            ctx.request_repaint();
        }
    }

    /// Drain offline dictation deltas. Local Whisper commits each chunk as it
    /// finishes, so every delta is final text.
    fn poll_local_dictation(&mut self, ctx: &egui::Context) {
        let mut events = Vec::new();
        if let Some(session) = self.local_dictation.as_ref() {
            while let Some(event) = session.poll() {
                let terminal =
                    matches!(event, DictationEvent::Finished | DictationEvent::Failed(_));
                events.push(event);
                if terminal {
                    break;
                }
            }
        }
        for event in events {
            match event {
                DictationEvent::Delta(text) => self.apply_dictation_delta(ctx, &text),
                DictationEvent::Interim(text) => self.apply_dictation_interim(ctx, &text),
                DictationEvent::Final(text) => self.apply_dictation_final(ctx, &text),
                DictationEvent::Finished => {
                    self.local_dictation = None;
                    self.dictation_stopping = false;
                    if let Some(stream) = self.dictation_stream.as_mut() {
                        stream.finished = true;
                    } else {
                        self.finish_transcription(ctx);
                    }
                    self.status_message = String::from("Dictation inserted.");
                }
                DictationEvent::Failed(err) => {
                    self.local_dictation = None;
                    self.dictation_stopping = false;
                    // Keep whatever was already transcribed; only report the
                    // failure instead of rolling the text back.
                    if let Some(stream) = self.dictation_stream.as_mut() {
                        stream.finished = true;
                    }
                    self.status_message = String::from("Local dictation stopped.");
                    self.push_toast(friendly_local_error(&err), true);
                    break;
                }
            }
        }
        if self.local_dictation.is_some() {
            ctx.request_repaint();
        }
    }

    /// Drain provider dictation deltas and finish the edit when it stops.
    fn poll_provider_dictation(&mut self, ctx: &egui::Context) {
        let mut events = Vec::new();
        if let Some(session) = self.provider_dictation.as_ref() {
            while let Some(event) = session.poll() {
                let terminal =
                    matches!(event, DictationEvent::Finished | DictationEvent::Failed(_));
                events.push(event);
                if terminal {
                    break;
                }
            }
        }
        for event in events {
            match event {
                DictationEvent::Delta(text) => self.apply_dictation_delta(ctx, &text),
                DictationEvent::Interim(text) => self.apply_dictation_interim(ctx, &text),
                DictationEvent::Final(text) => self.apply_dictation_final(ctx, &text),
                DictationEvent::Finished => {
                    self.provider_dictation = None;
                    self.dictation_stopping = false;
                    if let Some(stream) = self.dictation_stream.as_mut() {
                        stream.finished = true;
                    } else {
                        self.finish_transcription(ctx);
                    }
                    self.status_message = String::from("Dictation inserted.");
                }
                DictationEvent::Failed(err) => {
                    self.provider_dictation = None;
                    self.dictation_stopping = false;
                    if let Some(stream) = self.dictation_stream.as_mut() {
                        stream.finished = true;
                    }
                    self.status_message = String::from("Provider dictation stopped.");
                    self.push_toast(friendly_generation_error(&err, self.chat_provider), true);
                    break;
                }
            }
        }
        if self.provider_dictation.is_some() {
            ctx.request_repaint();
        }
    }

    /// Poll the local model download and report when it finishes.
    fn poll_whisper_download(&mut self, ctx: &egui::Context) {
        let mut outcome = None;
        let mut finished_model = None;
        if let Some(download) = self.whisper_download.as_mut() {
            while let Some(event) = download.poll() {
                match event {
                    local_stt::DownloadEvent::Progress { .. } => {}
                    local_stt::DownloadEvent::Ready => {
                        finished_model = Some(download.model);
                        outcome = Some(Ok(()));
                        break;
                    }
                    local_stt::DownloadEvent::Failed(err) => {
                        outcome = Some(Err(err));
                        break;
                    }
                }
            }
        }
        if let Some(result) = outcome {
            self.whisper_download = None;
            match result {
                Ok(()) => {
                    if let Some(model) = finished_model {
                        self.dictation_model = model.id.to_string();
                        self.save_assistant_settings();
                    }
                    self.whisper_error = None;
                    self.status_message = String::from("Local dictation model ready.");
                    self.push_toast("Local dictation model ready.", false);
                }
                Err(err) => {
                    self.whisper_error = Some(err.clone());
                    self.push_toast(err, true);
                }
            }
        }
        if self.whisper_download.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    /// Buffer a streamed chunk. [`Self::reveal_dictation`] feeds it into the
    /// document a few characters at a time.
    fn apply_dictation_delta(&mut self, ctx: &egui::Context, delta: &str) {
        if delta.is_empty() {
            return;
        }
        if self.dictation_stream.is_none() {
            self.begin_dictation_stream(ctx);
        }
        if let Some(stream) = self.dictation_stream.as_mut() {
            stream.pending.push_str(delta);
        }
    }

    /// Show an updated speculative transcript, replacing the previous interim.
    fn apply_dictation_interim(&mut self, ctx: &egui::Context, text: &str) {
        if text.is_empty() {
            return;
        }
        if self.dictation_stream.is_none() {
            self.begin_dictation_stream(ctx);
        }

        let mut cursor = 0;
        let mut changed = false;
        if let Some(stream) = self.dictation_stream.as_mut() {
            let separator = if !stream.inserted.is_empty()
                && !stream
                    .inserted
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_whitespace())
                && !text.chars().next().is_some_and(|c| c.is_whitespace())
            {
                " "
            } else {
                ""
            };
            let next = format!("{separator}{text}");
            if next == stream.interim {
                return;
            }

            let start = stream.start_b + stream.inserted.len();
            let end = start + stream.interim.len();
            if end > self.content.len() {
                return;
            }
            self.content.replace_range(start..end, &next);
            stream.interim = next;
            let end = start + stream.interim.len();
            cursor = self.content[..end].chars().count();
            changed = true;
        }

        if changed {
            self.refresh_stats();
            self.store_editor_cursor(ctx, cursor);
        }
    }

    /// Commit a finalized segment, replacing the current interim text.
    fn apply_dictation_final(&mut self, ctx: &egui::Context, text: &str) {
        if text.is_empty() {
            return;
        }
        if self.dictation_stream.is_none() {
            self.begin_dictation_stream(ctx);
        }

        let mut cursor = 0;
        let mut changed = false;
        if let Some(stream) = self.dictation_stream.as_mut() {
            let start = stream.start_b + stream.inserted.len();
            let end = start + stream.interim.len();
            if end > self.content.len() {
                return;
            }
            self.content.replace_range(start..end, "");
            stream.interim.clear();

            let separator = if !stream.inserted.is_empty()
                && !stream
                    .inserted
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_whitespace())
                && !text.chars().next().is_some_and(|c| c.is_whitespace())
            {
                " "
            } else {
                ""
            };
            let commit = format!("{separator}{text}");
            let at = stream.start_b + stream.inserted.len();
            self.content.insert_str(at, &commit);
            stream.inserted.push_str(&commit);
            let end = stream.start_b + stream.inserted.len();
            cursor = self.content[..end].chars().count();
            changed = true;
        }

        if changed {
            self.refresh_stats();
            self.store_editor_cursor(ctx, cursor);
        }
    }

    /// Move pending characters into the document each frame. Small deltas are
    /// shown immediately; large bursts are revealed progressively so text
    /// never pops in all at once.
    fn reveal_dictation(&mut self, ctx: &egui::Context) {
        let mut revealed = false;
        let mut cursor = 0;
        let mut done = false;

        if let Some(stream) = self.dictation_stream.as_mut() {
            let pending = stream.pending.chars().count();
            if pending > 0 {
                let take = if pending <= 32 {
                    pending
                } else {
                    (pending / 10).clamp(4, 16)
                };
                let split = stream
                    .pending
                    .char_indices()
                    .nth(take)
                    .map(|(index, _)| index)
                    .unwrap_or(stream.pending.len());
                let piece = stream.pending[..split].to_string();
                stream.pending.drain(..split);

                let at = stream.start_b + stream.inserted.len();
                self.content.insert_str(at, &piece);
                stream.inserted.push_str(&piece);
                let end = stream.start_b + stream.inserted.len();
                cursor = self.content[..end].chars().count();
                revealed = true;
            }
            done = stream.finished && stream.pending.is_empty();
        }

        if revealed {
            self.refresh_stats();
            self.store_editor_cursor(ctx, cursor);
        }
        if done {
            self.finish_transcription(ctx);
        }
    }

    /// Snapshot the document and remove the selection before the first delta.
    fn begin_dictation_stream(&mut self, ctx: &egui::Context) {
        let range = TextEditState::load(ctx, egui::Id::new("editor_text"))
            .and_then(|state| state.cursor.char_range());
        let original_content = self.content.clone();
        let (start_c, end_c) = match range {
            Some(range) => {
                let (a, b) = (range.primary.index, range.secondary.index);
                if a <= b { (a, b) } else { (b, a) }
            }
            None => {
                let end = self.content.chars().count();
                (end, end)
            }
        };
        let (prepared, start_b, after_needs_space) =
            prepare_dictation(&self.content, start_c, end_c);
        self.content = prepared;
        self.refresh_stats();
        self.dictation_stream = Some(DictationStream {
            original_content,
            original_range: range.map(|range| (range.primary.index, range.secondary.index)),
            start_b,
            inserted: String::new(),
            pending: String::new(),
            interim: String::new(),
            after_needs_space,
            finished: false,
        });
    }

    fn finish_transcription(&mut self, ctx: &egui::Context) {
        let Some(mut stream) = self.dictation_stream.take() else {
            self.status_message = String::from("No speech was detected.");
            self.focus_editor(ctx);
            return;
        };
        if !stream.interim.is_empty() {
            stream.inserted.push_str(&stream.interim);
            stream.interim.clear();
        }
        if stream.inserted.trim().is_empty() {
            self.content = stream.original_content;
            self.restore_editor_range(ctx, stream.original_range);
            self.refresh_stats();
            self.status_message = String::from("No speech was detected.");
            self.focus_editor(ctx);
            return;
        }

        if stream.after_needs_space {
            let at = stream.start_b + stream.inserted.len();
            self.content.insert(at, ' ');
        }
        let end = stream.start_b + stream.inserted.len();
        let cursor = self.content[..end].chars().count();
        self.store_editor_cursor(ctx, cursor);
        self.note_content_changed();
        self.status_message = if self.file_path.is_none() {
            String::from("Dictation inserted — save the file to keep it.")
        } else {
            String::from("Dictation inserted.")
        };
        self.focus_editor(ctx);
    }

    /// Stop the worker and roll back the partially streamed text.
    pub fn cancel_dictation(&mut self, ctx: &egui::Context) {
        if let Some(session) = self.local_dictation.take() {
            session.request_stop();
        }
        if let Some(session) = self.provider_dictation.take() {
            session.request_stop();
        }
        self.dictation_stopping = false;
        if let Some(stream) = self.dictation_stream.take() {
            self.content = stream.original_content;
            self.restore_editor_range(ctx, stream.original_range);
            self.refresh_stats();
        }
    }

    fn store_editor_cursor(&self, ctx: &egui::Context, char_index: usize) {
        let id = egui::Id::new("editor_text");
        if let Some(mut state) = TextEditState::load(ctx, id) {
            state
                .cursor
                .set_char_range(Some(CCursorRange::one(egui::text::CCursor::new(
                    char_index,
                ))));
            state.store(ctx, id);
        }
    }

    fn focus_editor(&self, ctx: &egui::Context) {
        ctx.memory_mut(|m| m.request_focus(egui::Id::new("editor_text")));
    }

    fn restore_editor_range(&self, ctx: &egui::Context, range: Option<(usize, usize)>) {
        let id = egui::Id::new("editor_text");
        if let Some(mut state) = TextEditState::load(ctx, id) {
            let range = range.map(|(a, b)| {
                CCursorRange::two(egui::text::CCursor::new(a), egui::text::CCursor::new(b))
            });
            state.cursor.set_char_range(range);
            state.store(ctx, id);
        }
    }

    /// Start microphone dictation with the selected source.
    pub fn start_dictation(&mut self, ctx: &egui::Context) {
        match self.dictation_source {
            DictationSource::Provider => self.start_provider_dictation(ctx),
            DictationSource::Local => self.start_local_dictation(ctx),
        }
    }

    fn start_provider_dictation(&mut self, ctx: &egui::Context) {
        if !provider_stt::supports(self.chat_provider) {
            self.report_error(format!(
                "{} doesn't offer transcription. Choose Local in Settings > Dictation or \
                 switch to Gemini or OpenAI in Settings > Provider.",
                self.chat_provider.label()
            ));
            return;
        }
        let api_key = self.api_key();
        if api_key.is_empty() {
            self.report_error(format!(
                "Add a {} API key in Settings > Provider to dictate.",
                self.chat_provider.label()
            ));
            return;
        }
        let model = provider_stt::transcription_model(self.chat_provider).to_string();
        if model.is_empty() {
            self.report_error("This provider has no transcription model.");
            return;
        }
        let provider = self.chat_provider;
        self.cancel_dictation(ctx);
        self.provider_dictation = Some(ProviderSession::start_microphone(provider, api_key, model));
        self.dictation_stopping = false;
        self.status_message = format!("Listening with {}…", provider.label());
    }

    fn start_local_dictation(&mut self, ctx: &egui::Context) {
        let Some(paths) = self.dictation_model_paths() else {
            self.report_error(
                "Download a dictation model in Settings > Dictation to use the microphone.",
            );
            return;
        };
        self.cancel_dictation(ctx);
        self.local_dictation = Some(LocalSession::start_microphone(paths));
        self.dictation_stopping = false;
        self.status_message = String::from("Listening… speak, then click to stop.");
    }

    /// Stop the microphone and let the worker transcribe its last audio.
    pub fn stop_dictation(&mut self) {
        if let Some(session) = self.local_dictation.as_ref() {
            session.request_stop();
            self.dictation_stopping = true;
            self.status_message = String::from("Finishing dictation…");
            return;
        }
        if let Some(session) = self.provider_dictation.as_ref() {
            session.request_stop();
            self.dictation_stopping = true;
            self.status_message = String::from("Finishing dictation…");
        }
    }

    /// Highlight the window while an audio file is dragged over it.
    pub fn show_audio_drop_overlay(&self, ctx: &egui::Context) {
        let hovering = ctx.input(|i| {
            i.raw.hovered_files.iter().any(|file| {
                file.path
                    .as_ref()
                    .is_some_and(|path| local_stt::is_audio_file(path))
            })
        });
        if !hovering {
            return;
        }

        let rect = ctx.screen_rect().shrink(16.0);
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("audio_drop_overlay"),
        ));
        painter.rect_filled(
            rect,
            10.0,
            egui::Color32::from_rgba_unmultiplied(ACCENT.r(), ACCENT.g(), ACCENT.b(), 24),
        );
        painter.rect_stroke(rect.shrink(1.0), 10.0, egui::Stroke::new(2.0_f32, ACCENT));

        let galley = painter.layout_no_wrap(
            String::from("Transcribe audio file"),
            egui::FontId::proportional(19.0),
            ACCENT_TEXT,
        );
        painter.galley(rect.center() - galley.size() * 0.5, galley, ACCENT_TEXT);
    }

    /// Transcribe the first audio file dropped anywhere on the window.
    pub fn handle_dropped_audio(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| {
            i.raw.dropped_files.iter().find_map(|file| {
                let path = file.path.as_ref()?;
                local_stt::is_audio_file(path).then(|| path.clone())
            })
        });
        let Some(path) = dropped else {
            return;
        };
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());

        match self.dictation_source {
            DictationSource::Provider => {
                if !provider_stt::supports(self.chat_provider) {
                    self.report_error(format!(
                        "{} doesn't offer transcription. Choose Local in Settings > Dictation \
                         or switch to Gemini or OpenAI in Settings > Provider.",
                        self.chat_provider.label()
                    ));
                    return;
                }
                let api_key = self.api_key();
                if api_key.is_empty() {
                    self.report_error(format!(
                        "Add a {} API key in Settings > Provider to transcribe audio.",
                        self.chat_provider.label()
                    ));
                    return;
                }
                let model = provider_stt::file_model(self.chat_provider, &self.chat_model);
                if model.is_empty() {
                    self.report_error("Pick a model in Settings > Provider to transcribe audio.");
                    return;
                }
                let provider = self.chat_provider;
                self.cancel_dictation(ctx);
                self.provider_dictation =
                    Some(ProviderSession::start_file(provider, api_key, model, path));
                self.dictation_stopping = false;
                self.status_message = format!("Transcribing {name}…");
            }
            DictationSource::Local => {
                let Some(paths) = self.dictation_model_paths() else {
                    self.report_error(
                        "Download a dictation model in Settings > Dictation to transcribe audio.",
                    );
                    return;
                };
                self.cancel_dictation(ctx);
                self.local_dictation = Some(LocalSession::start_file(paths, path));
                self.dictation_stopping = false;
                self.status_message = format!("Transcribing {name}…");
            }
        }
    }

    /// Persist the active provider, model, and dictation model.
    pub(crate) fn save_assistant_settings(&mut self) {
        let mut cfg = crate::app::config::load();
        cfg.provider = self.chat_provider.id().to_string();
        cfg.model = self.chat_model.trim().to_string();
        cfg.dictation_model = self.dictation_model.clone();
        cfg.dictation_source = self.dictation_source.id().to_string();
        if let Err(err) = crate::app::config::save(&cfg) {
            self.report_error(err);
        }
    }

    /// API key for the active provider from the OS keychain, falling back to
    /// the provider's environment variable.
    pub(crate) fn api_key(&self) -> String {
        if let Some(key) = crate::app::keys::load(self.chat_provider) {
            return key;
        }
        crate::app::config::pick("", self.chat_provider.env_var())
    }

    /// Send the instruction and the current document to the selected provider.
    fn send_chat(&mut self) {
        let instruction = self.chat_input.trim().to_string();
        if instruction.is_empty() || self.chat_busy || self.is_dictating() {
            return;
        }
        let api_key = self.api_key();
        if api_key.is_empty() {
            self.report_error("No API key: add one in Settings > AI.");
            return;
        }
        let model = self.chat_model.trim().to_string();
        if model.is_empty() {
            self.report_error("No model selected. Pick one in Settings > AI.");
            return;
        }
        let prompt = build_prompt(&instruction, &self.content);
        let (tx, rx) = mpsc::channel();
        let provider = self.chat_provider;
        // Reasoning models spend part of the budget before writing, so give
        // them a little more room than Gemini needs.
        let max_tokens = if provider == Provider::Gemini {
            2048
        } else {
            4096
        };
        self.chat_rx = Some(rx);
        self.chat_busy = true;
        self.chat_input.clear();
        self.status_message = format!("Asking {}…", provider.label());
        std::thread::spawn(move || {
            let _ = tx.send(generate_text(
                provider, &prompt, &api_key, &model, max_tokens, 0.7,
            ));
        });
    }

    /// Ask the assistant to design a styled HTML version of the document. The
    /// Markdown is left untouched; the HTML is kept in app state and written
    /// to a sidecar file next to the Markdown when possible.
    pub(crate) fn generate_styled_html(&mut self) {
        if self.html_busy || self.chat_busy || self.is_dictating() {
            return;
        }
        if self.content.trim().is_empty() {
            self.report_error("Open or write a document before styling it as HTML.");
            return;
        }
        let api_key = self.api_key();
        if api_key.is_empty() {
            self.report_error("No API key: add one in Settings > AI.");
            return;
        }
        let model = self.chat_model.trim().to_string();
        if model.is_empty() {
            self.report_error("No model selected. Pick one in Settings > AI.");
            return;
        }
        let prompt = build_html_design_prompt(&self.content);
        let (tx, rx) = mpsc::channel();
        let provider = self.chat_provider;
        let document = self.content.clone();
        let max_tokens = if provider == Provider::Gemini {
            8192
        } else {
            16384
        };
        self.html_rx = Some(rx);
        self.html_busy = true;
        self.html_error = None;
        self.html_started = Some(std::time::Instant::now());
        self.status_message = String::from("Creating a visual report...");
        std::thread::spawn(move || {
            let result = generate_text(provider, &prompt, &api_key, &model, max_tokens, 0.4)
                .and_then(|raw| {
                    let html = extract_html_document(&raw)?;
                    Ok(inject_fallback_image(
                        &resolve_commons_images(&html),
                        &document,
                    ))
                });
            let _ = tx.send(result);
        });
    }

    fn apply_styled_html(&mut self, html: String) {
        let html = match extract_html_document(&html) {
            Ok(html) => html,
            Err(message) => {
                self.status_message = String::from("Could not create the visual report.");
                self.html_error = Some(message.clone());
                self.html_started = None;
                self.push_toast(message, true);
                return;
            }
        };
        let html = mark_layout_tables(&html);
        let html = ensure_narrow_screen_support(&html);
        let html = litehtml::html::sanitize_html(&ensure_styled_marker(&html));
        self.styled_html = Some(html.clone());
        self.preview_html = true;
        self.preview_revision = self.preview_revision.wrapping_add(1);
        self.html_preview = None;
        self.html_error = None;
        self.html_started = None;

        if let Some(path) = self.styled_html_path() {
            match std::fs::write(&path, &html) {
                Ok(()) => {
                    self.status_message = String::from("Visual report ready.");
                }
                Err(_) => {
                    self.report_error("Could not save the visual report.");
                }
            }
        } else {
            self.status_message =
                String::from("Visual report ready. Save the document to keep a copy.");
        }
        self.push_toast("Visual report ready.", false);
    }

    fn styled_html_path(&self) -> Option<std::path::PathBuf> {
        let path = self.file_path.as_ref()?;
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("html"))
        {
            return None;
        }
        Some(path.with_extension("html"))
    }

    /// Load a styled HTML sidecar saved next to the current Markdown file.
    pub(crate) fn load_styled_html_sidecar(&mut self) {
        self.styled_html = None;
        self.html_preview = None;
        self.html_error = None;
        if let Some(path) = self.styled_html_path()
            && let Ok(html) = std::fs::read_to_string(path)
            && html.contains(STYLED_HTML_MARKER)
        {
            // Older sidecars may lack the narrow-screen rules; add them in
            // memory without rewriting the saved file.
            let html = ensure_narrow_screen_support(&mark_layout_tables(&html));
            self.styled_html = Some(litehtml::html::sanitize_html(&html));
        }
        self.preview_html = self.styled_html.is_some();
        self.preview_revision = self.preview_revision.wrapping_add(1);
    }

    pub fn show_chat_panel(&mut self, ctx: &egui::Context) {
        // Poll even when the Assistant panel is closed: visual reports can be
        // started from the toolbar while the user is in the editor/preview.
        self.poll_chat();
        self.poll_html();
        if self.chat_busy || self.html_busy {
            ctx.request_repaint();
        }
        if !self.tabs.assistant {
            return;
        }

        let has_key = self.has_api_key(self.chat_provider);
        let send_shortcut = ctx.format_shortcut(&egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND,
            egui::Key::Enter,
        ));

        egui::SidePanel::left("ai_chat_panel")
            .resizable(true)
            .default_width(ASSISTANT_DEFAULT_WIDTH)
            .min_width(ASSISTANT_MIN_WIDTH)
            // Cap the width so the panel can never be dragged wide enough
            // to hide the central editor entirely.
            .max_width((ctx.available_rect().width() - MIN_EDITOR_WIDTH).max(MIN_SIDE_PANEL_WIDTH))
            .frame(panel_frame(ctx))
            .show(ctx, |ui| {
                if panel_header(ui, "Assistant", "Close").clicked() {
                    self.tabs.assistant = false;
                }
                ui.separator();
                // Roomier spacing, and everything below scrolls so short
                // windows clip nothing instead of hiding content.
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                ui.spacing_mut().button_padding = egui::vec2(10.0, 5.0);
                let total_height = ui.available_height();
                let top_max = (total_height - 190.0).max(120.0);
                egui::ScrollArea::vertical()
                    .max_height(top_max)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.add_space(2.0);

                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Instruction").strong());
                            if !self.chat_input.is_empty() {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.small_button("Clear").clicked() {
                                            self.chat_input.clear();
                                        }
                                    },
                                );
                            }
                        });

                        let input = egui::TextEdit::multiline(&mut self.chat_input)
                            .hint_text("e.g. Turn these bullets into a table…")
                            .desired_rows(3)
                            .desired_width(f32::INFINITY)
                            .show(ui);
                        // Ctrl+Enter while typing sends. Checked against the raw
                        // input state (not `lost_focus`, which never fires here
                        // because Enter keeps focus in a multiline field).
                        if input.response.has_focus()
                            && ctx.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.command)
                        {
                            self.send_chat();
                        }

                        let can_send =
                            !self.chat_busy && !self.chat_input.trim().is_empty();

                        ui.horizontal_wrapped(|ui| {
                            let (send_text, send_color, send_fill) = if self.chat_busy {
                                // The leading spaces reserve room for the
                                // spinner that replaces the plane icon.
                                (
                                    String::from("       Thinking…"),
                                    TEXT,
                                    ui.visuals().widgets.inactive.weak_bg_fill,
                                )
                            } else {
                                (
                                    format!("{} Send  ({send_shortcut})", icons::PAPER_PLANE_TILT),
                                    ACCENT_TEXT,
                                    ACCENT,
                                )
                            };
                            let send = egui::Button::new(
                                egui::RichText::new(&send_text)
                                    .strong()
                                    .color(send_color),
                            )
                            .fill(send_fill);
                            let response = ui
                                .add_enabled(can_send, send)
                                .on_hover_text("Send the instruction to Gemini");

                            if self.chat_busy {
                                // A bare spinner sits where the plane icon was.
                                let galley = egui::WidgetText::from(
                                    egui::RichText::new(&send_text)
                                        .strong()
                                        .color(send_color),
                                )
                                .into_galley(
                                    ui,
                                    Some(false),
                                    f32::INFINITY,
                                    egui::TextStyle::Button,
                                );
                                let text_rect = ui.layout().align_size_within_rect(
                                    galley.size(),
                                    response.rect.shrink2(ui.spacing().button_padding),
                                );
                                let spinner_center = egui::pos2(
                                    text_rect.left() + 7.0,
                                    response.rect.center().y,
                                );
                                egui::Spinner::new().size(16.0).color(ACCENT).paint_at(
                                    ui,
                                    egui::Rect::from_center_size(
                                        spinner_center,
                                        egui::vec2(16.0, 16.0),
                                    ),
                                );
                            }

                            if response.clicked() {
                                self.send_chat();
                            }
                        });

                        if !has_key {
                            ui.weak("Add an API key in Settings > AI to use the assistant.");
                        } else {
                            ui.weak(
                                "Use the microphone in the editor toolbar to dictate into the document.",
                            );
                        }

                    });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Suggestion").strong());
                    if !self.chat_response.is_empty() {
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                ui.weak(
                                    egui::RichText::new(
                                        self.chat_provider.model_label(&self.chat_model),
                                    )
                                    .small(),
                                );
                            },
                        );
                    }
                });
                ui.add_space(4.0);

                egui::Frame::none()
                    .fill(BG_ELEVATED)
                    .rounding(8.0)
                    .stroke(egui::Stroke::new(1.0_f32, BORDER))
                    .inner_margin(egui::Margin::same(10.0))
                    .show(ui, |ui| {
                        let inner_height = ui.available_height();
                        ui.set_min_height(inner_height);
                        if self.chat_response.is_empty() {
                            ui.allocate_ui_with_layout(
                                egui::vec2(ui.available_width(), ui.available_height()),
                                egui::Layout::top_down(egui::Align::Center),
                                |ui| {
                                    ui.add_space(
                                        ((ui.available_height() - 46.0) * 0.5).max(8.0),
                                    );
                                    ui.label(
                                        egui::RichText::new(icons::SPARKLE)
                                            .size(22.0)
                                            .color(ui.visuals().weak_text_color()),
                                    );
                                    ui.add_space(2.0);
                                    ui.label(
                                        egui::RichText::new(
                                            "Gemini's suggestion will appear here.",
                                        )
                                        .weak()
                                        .small(),
                                    );
                                },
                            );
                        } else {
                            // Reserve the action row and a small gap, then let
                            // the response take the rest. The fixed math keeps
                            // the card inside the panel at every height.
                            ui.spacing_mut().item_spacing.y = 0.0;
                            let action_height = 30.0;
                            let gap = 6.0;
                            let body_height = (inner_height - action_height - gap).max(0.0);

                            ui.allocate_ui_with_layout(
                                egui::vec2(ui.available_width(), body_height),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    egui::ScrollArea::vertical()
                                        .auto_shrink([false; 2])
                                        .max_height(body_height)
                                        .show(ui, |ui| {
                                            ui.add_sized(
                                                egui::vec2(
                                                    ui.available_width(),
                                                    body_height,
                                                ),
                                                egui::TextEdit::multiline(
                                                    &mut self.chat_response,
                                                )
                                                .desired_width(f32::INFINITY)
                                                .frame(false)
                                                .code_editor(),
                                            );
                                        });
                                },
                            );
                            ui.add_space(gap);

                            let dictating = self.is_dictating();
                            ui.horizontal(|ui| {
                                if ui
                                    .add_enabled(!dictating, egui::Button::new("Insert"))
                                    .on_hover_text("Insert at cursor")
                                    .clicked()
                                {
                                    let snippet = self.chat_response.clone();
                                    insert_at_cursor(self, ctx, &snippet);
                                    self.note_content_changed();
                                }
                                if ui
                                    .add_enabled(!dictating, egui::Button::new("Replace"))
                                    .on_hover_text("Replace document")
                                    .clicked()
                                {
                                    self.content = self.chat_response.clone();
                                    self.note_content_changed();
                                }
                                if ui.button("Clear").clicked() {
                                    self.chat_response.clear();
                                }
                            });
                        }
                    });

            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dictation_inserts_with_spacing_between_words() {
        let (text, at, trailing) = prepare_dictation("hello world", 5, 5);
        assert_eq!(text, "hello  world");
        assert_eq!(at, 6);
        assert!(!trailing);
    }

    #[test]
    fn dictation_replaces_the_selection() {
        let (text, at, trailing) = prepare_dictation("keep this and that", 5, 9);
        assert_eq!(text, "keep  and that");
        assert_eq!(at, 5);
        assert!(!trailing);
        let mut streamed = text;
        streamed.insert_str(at, "only");
        assert_eq!(streamed, "keep only and that");
    }

    #[test]
    fn dictation_handles_reversed_and_empty_selection() {
        let (text, at, trailing) = prepare_dictation("ab", 2, 0);
        assert_eq!(text, "");
        assert_eq!(at, 0);
        assert!(!trailing);
        let (text, at, trailing) = prepare_dictation("", 0, 0);
        assert_eq!(text, "");
        assert_eq!(at, 0);
        assert!(!trailing);
    }

    #[test]
    fn dictation_reserves_a_trailing_space() {
        let (_, at, trailing) = prepare_dictation("ab", 1, 1);
        assert_eq!(at, 2);
        assert!(trailing);
    }

    #[test]
    fn friendly_errors_point_at_the_fix() {
        let gemini = Provider::Gemini;
        assert!(friendly_generation_error("403 API key not valid", gemini).contains("API key"));
        assert!(
            friendly_generation_error("429 RESOURCE_EXHAUSTED", gemini).contains("rate limiting")
        );
        assert!(
            friendly_generation_error("Request failed: dns error", gemini).contains("internet")
        );
        assert!(friendly_generation_error("weird thing", gemini).contains("weird thing"));
    }

    #[test]
    fn prompt_contains_instruction_and_document() {
        let p = build_prompt("Make it a table", "# Hi");
        assert!(p.contains("Make it a table"));
        assert!(p.contains("# Hi"));
        assert!(p.contains("Return ONLY"));
    }

    #[test]
    fn strips_markdown_fence_with_language_tag() {
        let out = strip_code_fence("```markdown\n# Hello\n```");
        assert_eq!(out, "# Hello");
    }

    #[test]
    fn strips_plain_fence_and_leaves_text_alone() {
        assert_eq!(strip_code_fence("```\nfoo\n```"), "foo");
        assert_eq!(strip_code_fence("# Hello"), "# Hello");
    }

    #[test]
    fn extracts_html_from_prose_and_fence() {
        let raw = "Here is the report.\n```html\n<!DOCTYPE html><html><body>hi</body></html>\n```\nEnjoy!";
        assert_eq!(
            extract_html_document(raw).unwrap(),
            "<!DOCTYPE html><html><body>hi</body></html>"
        );
    }

    #[test]
    fn rejects_prose_without_html() {
        assert!(extract_html_document("Here is a polished page").is_err());
        assert!(extract_html_document("<html><body>cut off").is_err());
    }

    #[test]
    fn finds_commons_image_placeholders() {
        let tag = "<img src='commons:finance office' alt='Finance office'>";
        assert_eq!(tag_commons_keywords(tag).as_deref(), Some("finance office"));
        assert_eq!(tag_commons_keywords("<img src='photo.jpg'>"), None);
    }

    #[test]
    fn images_render_as_full_width_aspect_ratio_boxes() {
        let image = CommonsImage {
            thumb_url: String::from("https://thumb.wikimedia.org/photo.jpg"),
            description_url: String::from("https://commons.wikimedia.org/wiki/File:Photo.jpg"),
            title: String::from("File:Photo.jpg"),
            license: String::from("CC0"),
            width: 960,
            height: 720,
        };
        let html = image_figure(&image, "Photo");
        assert!(html.contains("width:100%;padding-top:75.00%;"), "{html}");
        assert!(html.contains("background-size:cover"), "{html}");
        assert!(
            html.contains("background-image:url('https://thumb.wikimedia.org/photo.jpg')"),
            "{html}"
        );
        assert!(!html.contains("<img"), "{html}");
    }

    #[test]
    fn fallback_image_keywords_prefer_the_topic_word() {
        let queries = document_image_queries("# Solvik Coffee — Q3 2026 Review\n\nSome body text.");
        assert_eq!(queries.first().map(String::as_str), Some("solvik coffee"));
        assert_eq!(queries.get(1).map(String::as_str), Some("coffee"));
    }

    #[test]
    fn adds_narrow_screen_rules_when_missing() {
        let html = "<html><head><style>body{color:red;}</style></head><body><p>x</p></body></html>";
        let out = ensure_narrow_screen_support(html);
        assert!(out.contains("@media (max-width: 640px)"), "{out}");
        assert!(out.contains(".stack td"), "{out}");
        assert!(out.ends_with("</html>"));
    }

    #[test]
    fn keeps_narrow_screen_rules_when_present() {
        let html = "<html><head><style>@media (max-width: 640px) { .stack td { display: block; } }\
                    </style></head><body></body></html>";
        assert_eq!(ensure_narrow_screen_support(html), html);
    }

    #[test]
    fn stacks_card_tables_but_not_data_tables() {
        let html = "<table class=\"kpi-table\"><tr><td>one</td><td>two</td></tr></table>\
                    <table><tr><th>Metric</th><th>Value</th></tr><tr><td>a</td><td>b</td></tr></table>";
        let out = mark_layout_tables(html);
        assert!(out.contains("class=\"kpi-table stack\""), "{out}");
        assert!(!out.contains("<table class=\"stack\">"), "{out}");
        assert!(out.contains("<table><tr><th>Metric</th>"), "{out}");
    }

    #[test]
    fn parses_provider_responses() {
        let openai = serde_json::json!({
            "output": [{
                "type": "message",
                "content": [
                    { "type": "output_text", "text": "hello " },
                    { "type": "output_text", "text": "world" }
                ]
            }]
        });
        assert_eq!(openai_text(&openai).as_deref(), Some("hello world"));

        let anthropic = serde_json::json!({
            "content": [
                { "type": "thinking", "thinking": "hmm" },
                { "type": "text", "text": "answer" }
            ]
        });
        assert_eq!(anthropic_text(&anthropic).as_deref(), Some("answer"));

        let chat = serde_json::json!({
            "choices": [{ "message": { "content": "reply" } }]
        });
        assert_eq!(chat_completion_text(&chat).as_deref(), Some("reply"));
    }

    #[test]
    fn joins_text_parts_and_skips_thoughts() {
        let json = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [
                        {"thought": true, "text": "thinking..."},
                        {"text": "make it "},
                        {"text": "a table"}
                    ]
                }
            }]
        });
        assert_eq!(response_text(&json).as_deref(), Some("make it a table"));
    }
}
