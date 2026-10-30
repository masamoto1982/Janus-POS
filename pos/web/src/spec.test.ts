import { describe, expect, it } from "vitest";
import { ISSUES, SCENES } from "../../../docs/spec/3-screens";
import { renderScreens } from "../../../docs/spec/render";
import principles from "../../../docs/spec/2-principles.md?raw";

// 第三階層の対表（docs/spec/3-screens.ts）の書き方の検査と、読むための表の生成
describe("画面仕様書", () => {
  it("場面の番号は重ならない", () => {
    const ids = SCENES.map((s) => s.id);
    expect(ids.length).toBe(new Set(ids).size);
  });

  it("根拠のルール番号は第二階層にある", () => {
    const rules = new Set(principles.match(/P-\d{2}/g));
    const used = [...SCENES.flatMap((s) => s.rules), ...ISSUES.flatMap((q) => q.rules)];
    expect(used.filter((r) => !rules.has(r))).toEqual([]);
  });

  it("判断が要る点は、一覧にあり、どこかの場面から指している", () => {
    const known = new Set(ISSUES.map((q) => q.id));
    expect(SCENES.flatMap((s) => s.issues ?? []).filter((q) => !known.has(q))).toEqual([]);
    const used = new Set(SCENES.flatMap((s) => s.issues ?? []));
    expect(ISSUES.map((q) => q.id).filter((q) => !used.has(q))).toEqual([]);
  });

  it("3-screens.md はデータから生成したものと同じ（npx vitest run -u で書き直す）", async () => {
    await expect(renderScreens()).toMatchFileSnapshot("../../../docs/spec/3-screens.md");
  });
});
