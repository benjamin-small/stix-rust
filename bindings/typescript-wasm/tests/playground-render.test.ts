import { describe, it, expect } from "vitest";
import { Marked } from "marked";
import { escapeHtml, linkCitations, renderMarkdown } from "../playground/render.js";

const ID = "threat-actor--11111111-2222-4333-8444-555555555555";
const OTHER = "malware--11111111-2222-4333-8444-555555555556";
const known = new Set([ID]);

describe("render", () => {
  it("links known ids, with or without backticks", () => {
    expect(linkCitations(`*Zero Cool* (\`${ID}\`) and ${ID}`, known)).toBe(
      `*Zero Cool* ([\`${ID}\`](#obj:${ID})) and [\`${ID}\`](#obj:${ID})`,
    );
  });

  it("leaves unknown ids alone", () => {
    expect(linkCitations(`\`${OTHER}\``, known)).toBe(`\`${OTHER}\``);
  });

  it("renders Markdown with citation links", () => {
    const html = renderMarkdown(`## Cast\n\n*Zero Cool* (\`${ID}\`)`, Marked, known);
    expect(html).toContain("<h2");
    expect(html).toContain(`<a href="#obj:${ID}"><code>${ID}</code></a>`);
  });

  it("escapes raw HTML", () => {
    const html = renderMarkdown(`hello <script>alert(1)</script> <b>x</b>`, Marked, known);
    expect(html).not.toContain("<script>");
    expect(html).not.toContain("<b>");
    expect(html).toContain("&lt;script&gt;");
  });

  it("non-ASCII text survives", () => {
    const html = renderMarkdown(`Café 😀 — (\`${ID}\`)`, Marked, known);
    expect(html).toContain("Café 😀");
    expect(html).toContain(`#obj:${ID}`);
  });

  it("escapes HTML special characters", () => {
    expect(escapeHtml(`<a href="x">'&'</a>`)).toBe("&lt;a href=&quot;x&quot;&gt;&#39;&amp;&#39;&lt;/a&gt;");
  });
});
