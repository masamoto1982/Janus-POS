import { describe, expect, it } from "vitest";
import type { RegisterView } from "./types";
import { billCard, btn, callNotice, callScreen, esc, itemCount, regTop, sideOf } from "./view";

describe("view helpers", () => {
  it("escapes HTML", () => {
    expect(esc(`<a href="x">'&`)).toBe("&lt;a href=&quot;x&quot;&gt;&#39;&amp;");
  });

  it("disables a button with the core's reason", () => {
    const html = btn("会計", { type: "checkout" }, { action: "checkout", allowed: false, reason: "明細がありません" });
    expect(html).toContain("disabled");
    expect(html).toContain('title="明細がありません"');
  });

  it("routes notices to the side that acted", () => {
    expect(sideOf({ type: "scan", register: 1, side: "customer" })).toBe("customer");
    expect(sideOf({ type: "insert_cash", register: 1 })).toBe("customer");
    expect(sideOf({ type: "void_transaction", register: 1 })).toBe("staff");
    expect(sideOf({ type: "set_mat_loss", percent: 30 })).toBe("store");
  });

  it("lays out the top bar as item count, then total, then checkout", () => {
    const html = regTop(1280, 3, `<button>会計</button>`);
    const at = (t: string) => html.indexOf(t);
    expect(at("合計 ")).toBeGreaterThan(-1);
    expect(at("合計 ")).toBeLessThan(at("<b>3</b>点"));
    expect(at("<b>3</b>点")).toBeLessThan(at("1,280円"));
    expect(at("1,280円")).toBeLessThan(at("<button>会計"));
  });

  it("counts items by quantity", () => {
    expect(itemCount([])).toBe(0);
    expect(itemCount([{ qty: 2 }, { qty: 1 }])).toBe(3);
  });

  it("hides the top bar while nothing is registered", () => {
    expect(regTop(0, 0, `<button>会計</button>`)).toBe("");
  });

  it("marks the bill-payment confirmation as a customer-only button", () => {
    const items = [
      { name: "電気料金", amount: 5000, fee: 0 },
      { name: "水道料金", amount: 4000, fee: 110 },
    ];
    const html = billCard(1, items, 9110, { action: "confirm_bill_payment", allowed: true, reason: null });
    expect(html).toContain("customer-only");
    // 払込票ごとの内容・金額・手数料と、合計
    for (const t of ["電気料金", "5,000", "なし", "水道料金", "4,000", "110", "2件", "9,110"]) expect(html).toContain(t);
    expect(html).toContain("確認しました");
    expect(html).toContain("お支払いをやめる");
    expect(html).not.toContain("スキップ");
    // 取りやめ（白紙に戻る）は左下、確認しましたは右下
    expect(html.indexOf("お支払いをやめる")).toBeLessThan(html.indexOf("確認しました"));
    expect(sideOf({ type: "confirm_bill_payment", register: 1 })).toBe("customer");
  });

  it("shows the open calls only on the semi-self staff screen", () => {
    const call = (kind: string, created_at: number) => ({ id: 1, register: 1, kind, label: kind, created_at, detail: "" });
    const r = {
      staff: { calls: [call("fast_food", 1000), call("tobacco", 2000), call("fast_food", 3000)], mat_down: [] },
      customer: { staff_called: true },
    } as unknown as RegisterView;
    const staff = callScreen(r, 6000);
    expect(staff).toContain("FF＆煙草");
    expect(staff).toContain("5秒経過");
    expect(callNotice(r, 6000, "staff")).toBe("");
    const customer = callNotice(r, 6000, "customer");
    expect(customer).toBe("");
  });

  it("tells the customer that staff are on the way in attended mode", () => {
    const r = {
      mode: "attended",
      staff: { calls: [{ id: 1, register: 1, kind: "age_check", label: "年確", created_at: 1000, detail: "" }], mat_down: [] },
      customer: { staff_called: true },
    } as unknown as RegisterView;
    expect(callNotice(r, 6000, "customer")).toContain("店員が向かっています");
    expect(callNotice(r, 6000, "staff")).toBe("");
  });

  it("shows no elapsed time for an age-check call", () => {
    const r = {
      staff: { calls: [{ id: 1, register: 1, kind: "age_check", label: "年確", created_at: 1000, detail: "20歳以上" }], mat_down: [] },
      customer: { staff_called: true },
    } as unknown as RegisterView;
    expect(callNotice(r, 6000, "staff")).not.toContain("経過");
    expect(callScreen(r, 6000)).toContain("年確");
    expect(callScreen(r, 6000)).not.toContain("経過");
  });

  it("marks the item count when the limit is reached", () => {
    expect(regTop(100, 9, "", 10)).not.toContain("上限");
    expect(regTop(100, 10, "", 10)).toContain("上限");
  });
});
