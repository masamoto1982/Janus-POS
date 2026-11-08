import { describe, expect, it } from "vitest";
import { DEFAULT_LANG, langOf, loadLangs, saveLangs, toggleLang, trCore, trText } from "./i18n";

describe("screen language", () => {
  it("defaults to English and switches only the pressed side", () => {
    loadLangs(null);
    expect(DEFAULT_LANG).toBe("en");
    expect(toggleLang(1, "customer")).toBe("ja");
    expect(langOf(1, "customer")).toBe("ja");
    expect(langOf(1, "staff")).toBe("en");
    expect(langOf(2, "customer")).toBe("en");
    const saved = saveLangs();
    loadLangs(saved);
    expect(langOf(1, "customer")).toBe("ja");
    expect(toggleLang(1, "customer")).toBe("en");
  });

  it("translates fixed and variable phrases", () => {
    expect(trCore("バーコードをスキャンしてください")).toBe("Please scan a barcode");
    expect(trCore("お釣り 1,000円 をお返ししてください")).toBe("Return ¥1,000 in change");
    expect(trCore("2026年11月8日 日曜日 09:21")).toBe("Sun, Nov 8, 2026 09:21");
    expect(trCore("20歳以上の年齢確認をしますか？")).toBe("Check age (20+)?");
    expect(trCore("電子マネーの残高が足りません\nどうするか選んでください")).toBe("Not enough e-money balance\nPlease choose what to do");
  });

  it("keeps surrounding spaces and leaves unknown text as is", () => {
    expect(trText(" 合計 ")).toBe(" Total ");
    expect(trText("未知の文言")).toBe("未知の文言");
  });
});
