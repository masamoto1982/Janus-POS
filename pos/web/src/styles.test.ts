import { describe, expect, it } from "vitest";
import css from "./styles.css?raw";

/// 呼び出し中の黄の点滅を、色や段の塗り分けを組み直しても落とさないための見張り。
/// 2026-11-15 の色の組み直しで、セルフ操作モードのレジカウンター側の段（グレー）の規則が呼び出し中の黄の規則より強くなり、
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

  it("非常事態モードのヘッダーは赤と濃い黄を行き来する（四段目と同じ二色）", () => {
    const sos = rules.find(r => r.selector === ".screen-head.sos");
    expect(decl(sos!.body, "--head-call-bg")).toBe("var(--sos-head)");
    expect(decl(sos!.body, "--head-call-off")).toBe("var(--call-soft)");
    const sosRoot = rules.find(r => r.selector === ":root .screen.sos.sos.sos");
    const tokens = rules.filter(r => r.selector === ":root").map(r => r.body).join(";");
    // 四段目の消えている側（--tier-deep）は --alert-deep＝濃い黄と同じ色
    expect(decl(sosRoot!.body, "--tier-deep")).toBe("var(--alert-deep)");
    expect(decl(tokens, "--alert-deep")).toBe(decl(tokens, "--call-soft"));
  });

  it("薄い黄と濃い黄は同じ橙の濃度1と濃度2", () => {
    const tokens = rules.filter(r => r.selector === ":root").map(r => r.body).join(";");
    expect(decl(tokens, "--alert-bg")).toBe("var(--orange-1)");
    expect(decl(tokens, "--call-soft")).toBe("var(--orange-2)");
  });

  it("点滅していないレジ画面の呼び鈴は薄い黄（Bell エリアの端末は対象外）", () => {
    const tokens = rules.filter(r => r.selector === ":root").map(r => r.body).join(";");
    expect(decl(tokens, "--bell-bg")).toBe("var(--alert-bg)");
    const idle = rules.find(r => r.selector === ".screen .bell-btn:not(:disabled)");
    expect(decl(idle!.body, "background")).toBe("var(--bell-bg)");
    // Bell エリアの端末は対象外：待機中は背景を塗らない
    expect(rules.some(r => r.selector === ".bell-unit" && decl(r.body, "background"))).toBe(false);
  });

  it("呼び出し中のレジカウンター側の黄の段は、ほかの段の色の規則に負けない", () => {
    const calling = rules.findIndex(r => r.selector === ":root .staff-screen.calling");
    expect(decl(rules[calling].body, "--cust-bg")).toBe("var(--alert-bg)");
    // レジカウンター側に段の色を塗る規則は、呼び出し中を :not(.calling) で外すか、呼び出し中の規則より前にあって詳細度が同じ以下であること
    for (const [i, r] of rules.entries()) {
      if (i === calling || !decl(r.body, "--cust-bg") || !r.selector.includes("staff-screen") || r.selector.includes(".sos")) continue;
      expect(r.selector, `${r.selector} が呼び出し中の黄を上書きする`).toMatch(/:not\([^)]*\.calling/);
    }
  });
});

/// ボタンの形は四種（user 2026-11-16）：弱い角丸・強い角丸・タブ・円。セレクタと入力フォームはボタンではないので例外。
/// 範囲はレジ画面の中だけで、Item・Wallet など周辺機器とシミュレーターまわりは縛らない
describe("ボタンの形は四種", () => {
  const tokens = rules.filter(r => r.selector === ":root").map(r => r.body).join(";");
  const SHAPES = ["--shape-soft", "--shape-pill", "--shape-tab", "--shape-round"];

  it("形のトークンは四つだけ", () => {
    expect([...tokens.matchAll(/(--shape-[a-z-]+)\s*:/g)].map(m => m[1])).toEqual(SHAPES);
  });

  it("ボタンの角の丸みは四つのトークンのどれか（入力フォームの四角は例外）", () => {
    const outside = /\.item\b|\.picker|\.rules-dlg|\.bell-unit|\.bell-setting|\.build-badge/;
    const buttonish = /(^|[\s,>~+(])button\b|-btn\b|\.seg\b|\.tab\b|\.flag\b|\.ff-(item|arrow)\b|\.pad-key\b/;
    const checked = rules.filter(r => decl(r.body, "border-radius") !== undefined && buttonish.test(r.selector) && !outside.test(r.selector));
    expect(checked.length).toBeGreaterThan(5);
    for (const r of checked) {
      const v = decl(r.body, "border-radius")!;
      if (r.selector.includes("pad-field")) { expect(v, `${r.selector}：入力フォームは角を丸めない`).toBe("0"); continue; }
      expect(SHAPES.map(s => `var(${s})`), `${r.selector} の border-radius: ${v}`).toContain(v);
    }
  });

  it("FF のページ送り「<」「>」は円（user 2026-11-17：枠のない記号は五種目の形になっていた）", () => {
    const r = rules.find(r => r.selector === ".ff-arrow");
    expect(r && decl(r.body, "border-radius")).toBe("var(--shape-round)");
    expect(css).not.toMatch(/\.ff-arrow\s*\{[^}]*border:\s*none/);
  });
});

/// ボタンと入力欄の形（user 2026-11-17）：白銀比 1:√2 だけ。青銅比は退役。
/// 中身がうまく収まらないときだけ、同じ高さのまま幅をちょうど 2 倍か 3 倍にする
describe("ボタンの形は白銀比とその 2 倍・3 倍だけ", () => {
  it("青銅比が残っていない", () => {
    expect(css).not.toMatch(/--bronze|--sq-/);
  });

  it("幅の倍率は 2 か 3", () => {
    const factors = [...css.matchAll(/calc\(var\(--btn-(?:xl-|md-|sm-)?w(?:, [^)]*)?\) \* ([\d.]+)\)/g)].map((m) => m[1]);
    expect(factors.length).toBeGreaterThan(0);
    for (const f of factors) expect(["2", "3"]).toContain(f);
  });
});
