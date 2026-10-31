import { describe, expect, it } from "vitest";

import en from "./en";
import { resolveLanguage } from "./index";
import ru from "./ru";

function keys(obj: object, prefix = ""): string[] {
  return Object.entries(obj).flatMap(([k, v]) => (typeof v === "object" && v !== null ? keys(v, `${prefix}${k}.`) : [`${prefix}${k}`]));
}

describe("i18n", () => {
  it("has the same keys in every language", () => {
    expect(keys(en).sort()).toEqual(keys(ru).sort());
  });

  it("resolves the system language", () => {
    expect(resolveLanguage("system", "ru-RU")).toBe("ru");
    expect(resolveLanguage("system", "de-DE")).toBe("en");
    expect(resolveLanguage("en", "ru-RU")).toBe("en");
  });
});
