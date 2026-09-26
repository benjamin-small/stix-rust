// Source spans from the Rust core are UTF-8 byte offsets; JS strings index by
// UTF-16 code unit. These helpers convert between the two.

/** The UTF-16 index in `text` of UTF-8 byte offset `byte`, clamped to the end. */
export function byteToUtf16(text, byte) {
  let bytes = 0;
  let i = 0;
  while (i < text.length && bytes < byte) {
    const cp = text.codePointAt(i);
    bytes += cp < 0x80 ? 1 : cp < 0x800 ? 2 : cp < 0x10000 ? 3 : 4;
    i += cp > 0xffff ? 2 : 1;
  }
  return i;
}

/** A byte span as a UTF-16 `{start, end}` range into `text`. */
export function spanToRange(text, span) {
  return { start: byteToUtf16(text, span.start), end: byteToUtf16(text, span.end) };
}

// The `Display` format of stix-pattern's `ParseError`.
const PARSE_ERROR_RE = /parse error at bytes (\d+)\.\.(\d+): /;

/** The byte span a parse-error message names, or null if it names none. */
export function parseErrorSpan(message) {
  const m = PARSE_ERROR_RE.exec(message);
  return m ? { start: Number(m[1]), end: Number(m[2]) } : null;
}
