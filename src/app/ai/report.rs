//! Generated-report cleanup: extract the HTML document, resolve Commons
//! photos, and add the responsive rules the in-app renderer relies on.
/// Pull the first complete HTML document out of a model response, ignoring
/// any commentary or markdown fence around it.
pub(crate) fn extract_html_document(raw: &str) -> Result<String, String> {
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
    artist: String,
    license: String,
    width: u32,
    height: u32,
}

/// Overused placeholder words that produce generic stock-like photos instead
/// of something from the document. Queries made only of these are dropped so
/// the document-derived fallback image wins.
const ABSTRACT_IMAGE_WORDS: &[&str] = &[
    "earth",
    "planet",
    "world",
    "globe",
    "space",
    "satellite",
    "nature",
    "landscape",
    "environment",
    "background",
    "abstract",
    "concept",
    "business",
    "finance",
    "financial",
    "market",
    "growth",
    "success",
    "technology",
    "innovation",
    "digital",
    "data",
    "future",
    "ideas",
    "life",
];

fn abstract_image_query(keywords: &str) -> bool {
    let words: Vec<&str> = keywords
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect();
    !words.is_empty()
        && words
            .iter()
            .all(|word| ABSTRACT_IMAGE_WORDS.contains(&word.to_ascii_lowercase().as_str()))
}

/// File kinds that make poor hero photos unless the query explicitly asks for
/// one (for example a trail map in a hiking document).
const NON_PHOTO_WORDS: &[&str] = &[
    "map",
    "diagram",
    "chart",
    "graph",
    "logo",
    "schematic",
    "floor plan",
    "site plan",
    "icon",
    "seal",
    "coat of arms",
    "flag",
    "symbol",
];

fn non_photo_title(title: &str, words: &[String]) -> bool {
    let title = title.to_ascii_lowercase();
    let title_words = query_words(&title);
    NON_PHOTO_WORDS.iter().any(|word| {
        let asked = words.iter().any(|query| query.contains(word));
        let found = if word.contains(' ') {
            title.contains(word)
        } else {
            title_words.iter().any(|title_word| title_word == word)
        };
        found && !asked
    })
}

/// Search-result page, ordered by the relevance `index` the API assigns.
fn search_index(page: &serde_json::Value) -> u64 {
    page["index"].as_u64().unwrap_or(u64::MAX)
}

/// Whether a Commons file title mentions any meaningful word from the query.
/// Guards against a search that silently returns unrelated photos.
fn title_matches_query(title: &str, query_words: &[String]) -> bool {
    if query_words.is_empty() {
        return true;
    }
    let title = title.to_ascii_lowercase();
    query_words.iter().any(|word| title.contains(word))
}

fn query_words(keywords: &str) -> Vec<String> {
    keywords
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| word.len() > 2 && !word.chars().all(|c| c.is_ascii_digit()))
        .map(str::to_ascii_lowercase)
        .collect()
}

/// Replace `src="commons:<keywords>"` placeholders with public-domain
/// Wikimedia Commons photos and add a small credit line under each.
pub(crate) fn resolve_commons_images(html: &str) -> String {
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
            && !abstract_image_query(&keywords)
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

/// Crude HTML-tag stripper for the Commons artist field, which usually wraps
/// a user name in a link.
fn strip_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_tag = false;
    for ch in text.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.trim()
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn credit_html(image: &CommonsImage) -> String {
    let title = escape_html(image.title.strip_prefix("File:").unwrap_or(&image.title));
    let artist = if image.artist.is_empty() {
        String::new()
    } else {
        format!(" &middot; {}", escape_html(&image.artist))
    };
    format!(
        "<div style=\"font-size:11px;color:#9a9ca4;margin:4px 0 0;\">Photo: \
         <a href=\"{}\" style=\"color:#84aee0;\">{}</a>{artist} &middot; {}</div>",
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
    let words = query_words(keywords);
    let mut candidates: Vec<&serde_json::Value> = pages.values().collect();
    // The API returns pages keyed by page id, so restore the search ranking
    // before picking the first usable photo.
    candidates.sort_by_key(|page| search_index(page));
    for page in candidates {
        let info = &page["imageinfo"][0];
        let title = page["title"].as_str().unwrap_or("");
        if !title_matches_query(title, &words) || non_photo_title(title, &words) {
            continue;
        }
        let license = info["extmetadata"]["LicenseShortName"]["value"]
            .as_str()
            .unwrap_or("");
        let license_lower = license.to_ascii_lowercase();
        // Public domain needs no credit; CC BY and CC BY-SA are fine because
        // the photo block always prints the title, author and license.
        if !(license_lower.contains("cc0")
            || license_lower.contains("public domain")
            || license_lower == "pd"
            || license_lower.contains("cc by"))
        {
            continue;
        }
        let Some(thumb_url) = info["thumburl"].as_str() else {
            continue;
        };
        let description_url = info["descriptionurl"].as_str().unwrap_or("");
        if thumb_url.is_empty() || description_url.is_empty() || title.is_empty() {
            continue;
        }
        let artist = strip_tags(
            info["extmetadata"]["Artist"]["value"]
                .as_str()
                .unwrap_or(""),
        );
        return Some(CommonsImage {
            thumb_url: thumb_url.to_string(),
            description_url: description_url.to_string(),
            title: title.to_string(),
            artist,
            license: license.to_string(),
            width: info["thumbwidth"].as_u64().unwrap_or(0) as u32,
            height: info["thumbheight"].as_u64().unwrap_or(0) as u32,
        });
    }
    None
}

/// If the model didn't include any usable image, add a hero photo chosen from
/// the document's first heading so every visual report has one.
pub(crate) fn inject_fallback_image(html: &str, document: &str) -> String {
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
pub(crate) const STYLED_HTML_MARKER: &str = "<!-- rusty-notepad-styled-html -->";

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

pub(crate) fn ensure_styled_marker(html: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
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
    fn abstract_image_queries_are_rejected() {
        assert!(abstract_image_query("earth"));
        assert!(abstract_image_query("planet earth"));
        assert!(abstract_image_query("Business Success"));
        assert!(!abstract_image_query("finance office"));
        assert!(!abstract_image_query("kungsleden trail"));
        assert!(!abstract_image_query("mountain landscape"));
    }

    #[test]
    fn image_titles_must_mention_the_query() {
        let words = query_words("kungsleden trail");
        assert!(title_matches_query(
            "File:Kungsleden over Teusajaure.jpg",
            &words
        ));
        assert!(!title_matches_query(
            "File:North America from low orbiting satellite Suomi NPP.jpg",
            &words
        ));
    }

    #[test]
    fn non_photo_results_are_skipped_unless_requested() {
        let words = query_words("kungsleden");
        assert!(non_photo_title("File:Kungsleden map.png", &words));
        assert!(!non_photo_title(
            "File:Kungsleden over Teusajaure.jpg",
            &words
        ));
        let map_words = query_words("kungsleden map");
        assert!(!non_photo_title("File:Kungsleden map.png", &map_words));
    }

    #[test]
    fn images_render_as_full_width_aspect_ratio_boxes() {
        let image = CommonsImage {
            thumb_url: String::from("https://thumb.wikimedia.org/photo.jpg"),
            description_url: String::from("https://commons.wikimedia.org/wiki/File:Photo.jpg"),
            title: String::from("File:Photo.jpg"),
            artist: String::from("A. Photographer"),
            license: String::from("CC BY-SA 4.0"),
            width: 960,
            height: 720,
        };
        let html = image_figure(&image, "Photo");
        assert!(html.contains("width:100%;padding-top:75.00%;"), "{html}");
        assert!(html.contains("background-size:cover"), "{html}");
        assert!(html.contains("A. Photographer"), "{html}");
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
}
