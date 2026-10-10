// Markdown for the details panel. STIX id citations become links that focus the
// cited object (#obj:<id>); raw HTML in the source is escaped, never rendered.

const ID_RE = /`?([a-z][a-z0-9-]*--[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})`?/g;

const ESCAPES = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" };

export function escapeHtml(text) {
  return String(text).replace(/[&<>"']/g, (c) => ESCAPES[c]);
}

export function linkCitations(markdown, knownIds) {
  return markdown.replace(ID_RE, (match, id) => (knownIds.has(id) ? `[\`${id}\`](#obj:${id})` : match));
}

// Only these destinations become anchors; anything else renders as plain text.
const SAFE_HREF = /^(?:https?:|mailto:|#obj:|#report$)/i;

export function renderMarkdown(markdown, MarkedCtor, knownIds) {
  const marked = new MarkedCtor({
    gfm: true,
    renderer: {
      link(token) {
        const text = this.parser.parseInline(token.tokens);
        if (!SAFE_HREF.test(String(token.href).trim())) return text;
        const title = token.title ? ` title="${escapeHtml(token.title)}"` : "";
        return `<a href="${escapeHtml(token.href)}"${title}>${text}</a>`;
      },
      image: (token) => escapeHtml(token.text ?? ""),
      html: (token) => escapeHtml(typeof token === "string" ? token : token.text),
    },
  });
  return marked.parse(linkCitations(markdown ?? "", knownIds));
}
