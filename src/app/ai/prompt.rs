//! Prompt construction for the assistant.
/// Build the prompt sent to the assistant: user instruction + current document.
pub(crate) fn build_prompt(instruction: &str, document: &str) -> String {
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

/// Prompt for the optional assistant-designed HTML preview. The model reads the
/// document, decides what presentation suits it, and builds a self-contained
/// page within the renderer's constraints.
pub(crate) fn build_html_design_prompt(document: &str) -> String {
    let prompt = r#"You are a visual document designer. Read the Markdown below and design a polished, self-contained HTML page that helps a reader understand it at a glance.

THINK FIRST
- Decide what kind of document this is — an article, report, proposal, meeting notes, README or docs, plan, retrospective, recipe, letter, journal entry, research summary, and so on — and choose a presentation that fits it. Data-heavy documents may suit stat cards and charts; essays, stories and personal notes may suit typography, pull quotes and calm sections; docs may suit clear headings and a summary box. Never force a business-report layout onto text that has no numbers.
- Find the key message, structure, facts, numbers, comparisons and actions, then reorganize for clarity. Summarize where it helps, but keep the meaning and important details. Never invent or estimate data, and only chart numbers that appear in the source.
- Pick a handful of elements that genuinely help — cards, callouts, quotes, tables, timelines, bars, columns, a pie/donut — and skip the rest.
- Aim for roughly 600-1000 words: focused and compact, not a restatement of the source.

OUTPUT
- Return ONLY the complete HTML document, starting with <!DOCTYPE html>. No code fences, no commentary.
- One inline <style> block. No JavaScript, external stylesheets or fonts.
- Images: use placeholder URLs of the form `commons:<2-4 descriptive words>` (for example `<img src='commons:coffee cart' alt='Coffee cart'>`). Include one relevant hero photo near the top and optionally one more; the app resolves them to freely licensed Wikimedia Commons photos and adds the credit line. Choose a concrete, photographable subject named or clearly implied by the document — a place, trail, building, object, activity, dish or person. Never use abstract or distant words like "earth", "space", "nature", "business" or "success": the photo must show something specific the document talks about. Never rely on an image to convey data.

DESIGN
- Aim to wow on the first scroll, then keep it effortless to read. Build a dramatic hero: a full-width band with a subtle linear-gradient, a kicker chip, a display title up to 44px and a one-line summary.
- Give the page rhythm: oversized section numbers (01, 02, ...), thin rules, alternating tinted bands, editorial pull quotes, large stat numerals and small uppercase labels. Use filled panels, soft corners and gradients instead of outlines.
- Avoid a dashboard of identical rounded cards. Vary the composition section by section and let whitespace do work.
- Commit to one visual direction that suits the document — for example editorial magazine, field notes, or a technical brief — and carry it through type, spacing and imagery. Do not blend several looks.

ENGINE LIMITS
The preview is a lightweight HTML engine, not a browser:
- No CSS grid, flexbox, gap, box-shadow, transform, clamp(), calc(), aspect-ratio, inset, vw/vh, position:fixed or position:sticky. position:relative/absolute is allowed only for pie/donut markup.
- Build multi-column rows with `<table width="100%" style="table-layout:fixed">` and one `<td>` per column. Put card backgrounds, padding and rounding on the `<td>` itself, and use padding and margins for spacing.
- No min-width on tables. Data tables: at most 4 columns, no fixed layout, class="hide-narrow" on optional columns. Avoid long unbroken words and `white-space:nowrap` on text.
- Font sizes in px: body 14-15, a hero display title up to 44, h1 26-32, h2 18-22, h3 15-17, small labels 11-12; line-height around 1.5.
- Bars: a label/value table row, then a rounded track div with a child div sized by width. Columns: bottom-aligned table cells with fixed-height bars. Pie/donut: a conic-gradient on a fixed-size square div plus a legend table, using the exact shares from the source.
- The preview can be 320px wide: give layout tables class="stack" and chart cells class="chart-cell", and end the stylesheet with:
  @media (max-width: 640px) {
    body { padding: 14px 12px; font-size: 14px; }
    h1 { font-size: 22px; } h2 { font-size: 17px; margin: 18px 0 10px; } h3 { font-size: 15px; }
    .stack { border-spacing: 0; margin: 0; } .stack td { display: block; width: 100%; margin-bottom: 8px; }
    .chart-cell { display: block; width: 100%; }
    .hide-narrow { display: none; }
    .data { font-size: 12px; } .data th, .data td { padding: 6px 4px; }
  }
- Stacked card tables keep desktop gaps with `border-collapse:separate; border-spacing:9px 0; margin:0 -9px`.

PALETTE
Dark theme: page background #1c1d20, body text #e4e4e7, headings #ffffff, accent and links #84aee0, muted text #9a9ca3, dividers #3a3c44, raised cards #232428, positive #7ecb8f, warning #e0b878, negative #e08a8a.

--- MARKDOWN ---
"#;
    format!("{prompt}{document}")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prompt_contains_instruction_and_document() {
        let p = build_prompt("Make it a table", "# Hi");
        assert!(p.contains("Make it a table"));
        assert!(p.contains("# Hi"));
        assert!(p.contains("Return ONLY"));
    }

    #[test]
    fn html_prompt_embeds_document_and_engine_limits() {
        let p = build_html_design_prompt("# Notes");
        assert!(p.contains("# Notes"));
        assert!(p.contains("<!DOCTYPE html>"));
        assert!(p.contains("conic-gradient"));
        assert!(p.contains("@media (max-width: 640px)"));
    }
}
