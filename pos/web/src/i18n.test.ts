import { describe, expect, it } from "vitest";
import { DEFAULT_LANG, langOf, resetLangs, toggleLang, trCore, trText } from "./i18n";

describe("screen language", () => {
  it("defaults to Japanese, switches only the pressed side, and resets per register", () => {
    expect(DEFAULT_LANG).toBe("ja");
    expect(toggleLang(1, "customer")).toBe("en");
    expect(langOf(1, "customer")).toBe("en");
    expect(langOf(1, "staff")).toBe("ja");
    expect(langOf(2, "customer")).toBe("ja");
    toggleLang(2, "staff");
    resetLangs(1);
    expect(langOf(1, "customer")).toBe("ja");
    expect(langOf(2, "staff")).toBe("en");
    resetLangs(2);
  });

  it("translates fixed and variable phrases", () => {
    expect(trCore("バーコードをスキャンしてください")).toBe("Please scan a barcode");
    expect(trCore("お釣り 1,000円 をお返ししてください")).toBe("Return ¥1,000 in change");
    expect(trCore("2026年11月8日 日曜日 09:21")).toBe("Sun, Nov 8, 2026 09:21");
    expect(trCore("20歳以上の年齢確認をしますか？")).toBe("Check age (20+)?");
    expect(trCore("クレジットカードを決済端末にかざしてください")).toBe("Hold your credit card over the payment terminal");
    expect(trCore("電子マネーの残高が足りません\nどうするか選んでください")).toBe("Not enough e-money balance\nPlease choose what to do");
    expect(trCore("ポイント 1,250P（今回 +3P）")).toBe("Points 1,250P (+3P this time)");
    expect(trCore("ご利用 80P、付与 2P、ポイント残高 2P")).toBe("Used 80P, earned 2P, point balance 2P");
    expect(trCore("付与 3P、ポイント残高 1,253P")).toBe("Earned 3P, point balance 1,253P");
    expect(trCore("用件：FF・たばこ")).toBe("Called for: FF, tobacco");
  });

  it("keeps surrounding spaces and leaves unknown text as is", () => {
    expect(trText(" 合計 ")).toBe(" Total ");
    expect(trText("未知の文言")).toBe("未知の文言");
  });
});
