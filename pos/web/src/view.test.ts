import { describe, expect, it } from "vitest";
import type { RegisterView } from "./types";
import { billCard, btn, callNotice, esc, itemCount, regTop, sideOf } from "./view";

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

  it("lays out the top bar as item count, total, checkout from the left", () => {
    const html = regTop(1280, 3, `<button>会計</button>`);
    const at = (t: string) => html.indexOf(t);
    expect(at("3</b> 点")).toBeGreaterThan(-1);
    expect(at("3</b> 点")).toBeLessThan(at("合計"));
    expect(at("合計")).toBeLessThan(at("<button>会計"));
  });

  it("counts items by quantity", () => {
    expect(itemCount([])).toBe(0);
    expect(itemCount([{ qty: 2 }, { qty: 1 }])).toBe(3);
  });

  it("hides the top bar while nothing is registered", () => {
    expect(regTop(0, 0, `<button>会計</button>`)).toBe("");
  });

  it("marks the bill-payment confirmation as a customer-only button", () => {
    const html = billCard(1, 8640, { action: "confirm_bill_payment", allowed: true, reason: null });
    expect(html).toContain("customer-only");
    expect(html).toContain("8,640");
    expect(sideOf({ type: "confirm_bill_payment", register: 1 })).toBe("customer");
  });

  it("shows the staff which calls are open, and the customer a fixed waiting message", () => {
    const call = (kind: string, created_at: number) => ({ id: 1, register: 1, kind, label: kind, created_at, detail: "" });
    const r = {
      staff: { calls: [call("fast_food", 1000), call("tobacco", 2000), call("fast_food", 3000)], mat_down: [] },
      customer: { staff_called: true },
    } as unknown as RegisterView;
    const staff = callNotice(r, 6000, "staff");
    expect(staff).toContain("FF＆煙草");
    expect(staff).toContain("5秒経過");
    const customer = callNotice(r, 6000, "customer");
    expect(customer).toContain("係員を呼び出しています。しばらくお待ちください。");
    expect(customer).not.toContain("FF");
  });

  it("marks the item count when the limit is reached", () => {
    expect(regTop(100, 9, "", 10)).not.toContain("上限");
    expect(regTop(100, 10, "", 10)).toContain("上限");
  });
});
