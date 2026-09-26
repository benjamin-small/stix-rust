import { describe, it, expect } from "vitest";
import { Engine, ParseError } from "../dist/index.js";
import { byteToUtf16, spanToRange, parseErrorSpan } from "../playground/spans.js";

describe("byteToUtf16", () => {
  it("is the identity on ASCII", () => {
    expect(byteToUtf16("[file:size > 1]", 6)).toBe(6);
  });

  it("accounts for multi-byte characters", () => {
    // "[file:name = '" is 14 bytes and 14 units; "é" is 2 bytes / 1 unit;
    // "😀" is 4 bytes / 2 units.
    const s = "[file:name = 'é😀x']";
    expect(byteToUtf16(s, 14)).toBe(14);
    expect(byteToUtf16(s, 16)).toBe(15);
    expect(byteToUtf16(s, 20)).toBe(17);
    expect(byteToUtf16(s, 21)).toBe(18);
  });

  it("clamps offsets past the end", () => {
    expect(byteToUtf16("abc", 99)).toBe(3);
  });

  it("converts a span to a range", () => {
    expect(spanToRange("é = 1", { start: 0, end: 2 })).toEqual({ start: 0, end: 1 });
  });
});

describe("parseErrorSpan", () => {
  it("reads the span out of a real parse error", () => {
    let message = "";
    try {
      new Engine().parsePattern("[file:size >");
    } catch (e) {
      expect(e).toBeInstanceOf(ParseError);
      message = (e as Error).message;
    }
    expect(parseErrorSpan(message)).toEqual({ start: 12, end: 12 });
  });

  it("also reads the raw wasm message with its [parse] prefix", () => {
    expect(parseErrorSpan("[parse] parse error at bytes 3..7: nope")).toEqual({ start: 3, end: 7 });
  });

  it("returns null for a message with no span", () => {
    expect(parseErrorSpan("something else")).toBeNull();
  });
});
