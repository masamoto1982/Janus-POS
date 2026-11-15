import { describe, expect, it } from "vitest";
import css from "./styles.css?raw";

/// 呼び出し中の黄の点滅を、色や段の塗り分けを組み直しても落とさないための見張り。
/// 2026-11-15 の色の組み直しで、セルフ操作モードのスタッフ側の段（グレー）の規則が呼び出し中の黄の規則より強くなり、
/// ヘッダーが黄とグレーの点滅に化けた（以前に一度直したもの）。

const rules = [...css.replace(/\/\*[\s\S]*?\*\//g, "").matchAll(/([^{}]+)\{([^{}]*)\}/g)].map(m => ({ selector: m[1].trim(), body: m[2] }));
const decl = (body: string, prop: string) => body.match(new RegExp(`${prop}\\s*:\\s*([^;]+)`))?.[1].trim();

describe("呼び出し中の黄の点滅", () => {
  it("ヘッダーは濃い黄と薄い黄を行き来する（消えている側を段の色に任せない）", () => {
    const calling = rules.find(r => r.selector === ".screen-head.calling");
    expect(decl(calling!.body, "--head-call-bg")).toBe("var(--call-soft)");
    expect(decl(calling!.body, "--head-call-off")).toBe("var(--alert-bg)");
    const blink = rules.find(r => r.selector === ".screen-head:is(.calling, .sos)");
    expect(decl(blink!.body, "background")).toBe("color-mix(in oklch, var(--head-call-bg) calc(var(--blink-on) * 100%), var(--head-call-off))");
  });

  it("薄い黄と濃い黄は同じ橙の濃度1と濃度2", () => {
    const tokens = rules.filter(r => r.selector === ":root").map(r => r.body).join(";");
    expect(decl(tokens, "--alert-bg")).toBe("var(--orange-1)");
    expect(decl(tokens, "--call-soft")).toBe("var(--orange-2)");
  });

  it("呼び出し中のスタッフ側の黄の段は、ほかの段の色の規則に負けない", () => {
    const calling = rules.findIndex(r => r.selector === ":root .staff-screen.calling");
    expect(decl(rules[calling].body, "--cust-bg")).toBe("var(--alert-bg)");
    // スタッフ側に段の色を塗る規則は、呼び出し中を :not(.calling) で外すか、呼び出し中の規則より前にあって詳細度が同じ以下であること
    for (const [i, r] of rules.entries()) {
      if (i === calling || !decl(r.body, "--cust-bg") || !r.selector.includes("staff-screen") || r.selector.includes(".sos")) continue;
      expect(r.selector, `${r.selector} が呼び出し中の黄を上書きする`).toMatch(/:not\([^)]*\.calling/);
    }
  });
});
