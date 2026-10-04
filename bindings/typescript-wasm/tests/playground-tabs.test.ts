import { describe, it, expect } from "vitest";
import { nextTabIndex } from "../playground/tabs.js";

describe("nextTabIndex", () => {
  it("moves right and wraps", () => {
    expect(nextTabIndex(0, "ArrowRight", 5)).toBe(1);
    expect(nextTabIndex(4, "ArrowRight", 5)).toBe(0);
  });

  it("moves left and wraps", () => {
    expect(nextTabIndex(3, "ArrowLeft", 5)).toBe(2);
    expect(nextTabIndex(0, "ArrowLeft", 5)).toBe(4);
  });

  it("jumps with Home and End", () => {
    expect(nextTabIndex(2, "Home", 5)).toBe(0);
    expect(nextTabIndex(2, "End", 5)).toBe(4);
  });

  it("ignores other keys", () => {
    expect(nextTabIndex(2, "Enter", 5)).toBeNull();
    expect(nextTabIndex(2, "ArrowDown", 5)).toBeNull();
  });
});
