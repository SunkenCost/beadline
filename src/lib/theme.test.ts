import { describe, expect, it } from "vitest";
import { isSafeCssColor, parseThemeJson, resolveAppearance } from "./theme";

describe("theme validation & resolution", () => {
  it("resolves system appearance cleanly", () => {
    expect(resolveAppearance("system", true)).toBe("dark");
    expect(resolveAppearance("system", false)).toBe("light");
    expect(resolveAppearance("dark", false)).toBe("dark");
    expect(resolveAppearance("light", true)).toBe("light");
  });

  it("validates safe CSS colors and rejects CSS injection", () => {
    expect(isSafeCssColor("#D97757")).toBe(true);
    expect(isSafeCssColor("rgba(255, 255, 255, 0.08)")).toBe(true);
    expect(isSafeCssColor("oklch(0.65 0.14 38)")).toBe(true);
    expect(isSafeCssColor("url(javascript:alert(1))")).toBe(false);
    expect(isSafeCssColor("#fff; background: red")).toBe(false);
  });

  it("parses a valid BeadlineThemeV1 payload", () => {
    const res = parseThemeJson(
      JSON.stringify({
        schemaVersion: 1,
        id: "nord-frost",
        name: "Nord Frost",
        appearance: "dark",
        tokens: {
          bg: "#2e3440",
          accent: "#88c0d0",
        },
      }),
    );
    expect(res.ok).toBe(true);
    if (res.ok) {
      expect(res.theme.id).toBe("nord-frost");
      expect(res.theme.tokens.accent).toBe("#88c0d0");
    }
  });

  it("rejects unknown tokens or invalid schema versions", () => {
    const badVersion = parseThemeJson(JSON.stringify({ schemaVersion: 2 }));
    expect(badVersion.ok).toBe(false);

    const badToken = parseThemeJson(
      JSON.stringify({
        schemaVersion: 1,
        id: "bad-theme",
        name: "Bad",
        appearance: "dark",
        tokens: { "evil-prop": "#fff" },
      }),
    );
    expect(badToken.ok).toBe(false);
  });
});
