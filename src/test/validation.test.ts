import { describe, expect, it } from "vitest";

import { hostOf, isValidUrl, parseLinks } from "../lib/validation";

describe("isValidUrl", () => {
  it("rejects empty input", () => {
    expect(isValidUrl("")).toBe(false);
    expect(isValidUrl("   ")).toBe(false);
  });

  it("rejects malformed links", () => {
    expect(isValidUrl("not a url")).toBe(false);
    expect(isValidUrl("ftp://example.com/file")).toBe(false);
    expect(isValidUrl("javascript:alert(1)")).toBe(false);
  });

  it("accepts http and https links", () => {
    expect(isValidUrl("https://example.com/a")).toBe(true);
    expect(isValidUrl("http://example.com/a")).toBe(true);
    expect(isValidUrl("example.com/a")).toBe(true);
  });
});

describe("parseLinks", () => {
  it("returns nothing for empty input", () => {
    expect(parseLinks("")).toEqual({ links: [], invalid: [] });
  });

  it("trims whitespace and ignores blank lines", () => {
    const result = parseLinks("  https://example.com/a  \n\n \nhttps://example.com/b\n");
    expect(result.links.map((link) => link.url)).toEqual([
      "https://example.com/a",
      "https://example.com/b",
    ]);
    expect(result.invalid).toHaveLength(0);
  });

  it("separates invalid lines from valid ones", () => {
    const result = parseLinks("https://example.com/a\nnonsense\n");
    expect(result.links).toHaveLength(1);
    expect(result.invalid).toHaveLength(1);
    expect(result.invalid[0].raw).toBe("nonsense");
  });
});

describe("hostOf", () => {
  it("drops the www prefix", () => {
    expect(hostOf("https://www.example.com/a")).toBe("example.com");
  });

  it("returns an empty string for junk", () => {
    expect(hostOf("nonsense")).toBe("");
  });
});
