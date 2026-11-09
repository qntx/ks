import { describe, expect, it } from "vite-plus/test";

import { joinName, suggestName } from "./name";

describe("name", () => {
  it("joins folder and leaf", () => {
    expect(joinName("", "a")).toBe("a");
    expect(joinName("work", "a")).toBe("work/a");
  });

  it("suggests names from file names", () => {
    expect(suggestName("", "My Passport.PDF")).toBe("my-passport.pdf");
    expect(suggestName("personal", "My Passport.PDF")).toBe("personal/my-passport.pdf");
    expect(suggestName("", "a.age")).toBe("a");
    expect(suggestName("", "....")).toBe("file");
    expect(suggestName("", "café notes.txt")).toBe("caf-notes.txt");
    expect(suggestName("", "/tmp/x/Scan 1.PNG")).toBe("scan-1.png");
  });
});
