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

/// Prompt for the optional assistant-designed HTML preview. The model is asked
/// to analyze the document and build a visual report, not just reformat it.
pub(crate) fn build_html_design_prompt(document: &str) -> String {
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
}
