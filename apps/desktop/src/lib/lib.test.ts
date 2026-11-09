import { describe, expect, it } from "vite-plus/test";

import { formatBytes, groupDigits } from "./format";
import { fuzzyFilter, fuzzyScore } from "./fuzzy";

describe("fuzzy", () => {
  it("scores subsequences, best first", () => {
    expect(fuzzyScore("gh", "work/github")).toBeGreaterThanOrEqual(0);
    expect(fuzzyScore("zz", "work/github")).toBe(-1);
    const out = fuzzyFilter("aws", ["work/aws/root", "work/github", "work/aws/staging"]);
    expect(out).toStrictEqual(["work/aws/root", "work/aws/staging"]);
  });
});

describe("format", () => {
  it("formats bytes", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1200)).toBe("1.2 KB");
    expect(formatBytes(248102)).toBe("242.3 KB");
    expect(formatBytes(3 * 1024 * 1024)).toBe("3 MB");
  });

  it("groups digits", () => {
    expect(groupDigits("123456")).toBe("123 456");
    expect(groupDigits("12345678")).toBe("123 456 78");
  });
});
