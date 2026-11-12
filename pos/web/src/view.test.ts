import { describe, expect, it } from "vitest";
import type { RegisterView } from "./types";
import { billHandover, billPayCard, billVerifyScreen, btn, stampVerifyScreen, callScreen, esc, itemCount, menuPaused, regTop, sideOf, signage, SIGNAGE, SIGNAGE_MS } from "./view";

describe("view helpers", () => {
  it("keeps the signage out of the language switch", () => {
    expect(signage(0)).toContain("data-i18n-skip");
  });

  it("shows the customer side as closed in the first-wait style while the store menu is open", () => {
    const html = menuPaused();
    expect(html).toContain('<b class="mode-big">休止中</b>');
    expect(html).toContain(">Please use another register<");
    expect(html).toContain("空いているレジをお使いください");
    expect(html).toContain("data-i18n-skip");
    expect(html).not.toContain("<button");
  });

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
    expect(sideOf({ type: "set_bell_loss", percent: 30 })).toBe("store");
  });

  it("lays out the top bar as item count, then total, then checkout", () => {
    const html = regTop(1280, 3, `<button>会計</button>`);
    const at = (t: string) => html.indexOf(t);
    expect(at("合計 ")).toBeGreaterThan(-1);
    expect(at("合計 ")).toBeLessThan(at("<b>3</b>点"));
    expect(at("<b>3</b>点")).toBeLessThan(at("1,280</b>"));
    expect(at("1,280</b>")).toBeLessThan(at("<button>会計"));
  });

  it("counts items by quantity", () => {
    expect(itemCount([])).toBe(0);
    expect(itemCount([{ qty: 2 }, { qty: 1 }])).toBe(3);
  });

  it("hides the top bar while nothing is registered", () => {
    expect(regTop(0, 0, `<button>会計</button>`)).toBe("");
  });

  it("lists every utility bill slip with its payee and amount to tick, and shows the slip count large", () => {
    const html = billVerifyScreen(
      1,
      "staff",
      [
        { line_id: 1, code: "2100000000021", name: "電気料金", amount: 50000, slip_no: "9100 1234 5678 9012", checked: true, stamp: 200, stamped: false },
        { line_id: 2, code: "2100000000050", name: "ガス料金", amount: 3000, slip_no: "9100 2222 3333 4444", checked: false, stamp: 0, stamped: false },
        { line_id: 3, code: "2100000000021", name: "電気料金", amount: 50000, slip_no: "9100 5555 6666 7777", checked: false, stamp: 200, stamped: true },
      ],
      { action: "check_bill", allowed: true, reason: null },
    );
    for (const h of ["事業者", "金額", "バーコード番号", "確認"]) expect(html).toContain(`>${h}<`);
    expect(html).toContain("9100 2222 3333 4444");
    expect(html).not.toContain(">枚数<");
    expect(html.match(/電気料金/g)?.length).toBe(4);
    expect(html).toContain('type="checkbox" checked data-bill-check="1:staff:1"');
    expect(html).toContain('type="checkbox"  data-bill-check="1:staff:3"');
    expect(html).toContain(`<span class="bill-n">3枚</span>`);
    expect(html).toContain("合計 103,000円");
    const pay = billPayCard(3);
    expect(pay).toContain(`公共料金<span class="bill-n">3枚</span>を受け付けます</div>`);
    expect(pay).toContain("現金でお支払いください</p>");
  });

  it("lists only the slips that need a revenue stamp and ticks each one", () => {
    const bills = [
      { line_id: 1, code: "2100000000021", name: "電気料金", amount: 50000, slip_no: "9100 1234 5678 9012", checked: true, stamp: 200, stamped: false },
      { line_id: 2, code: "2100000000050", name: "ガス料金", amount: 3000, slip_no: "9100 2222 3333 4444", checked: true, stamp: 0, stamped: false },
    ];
    const html = stampVerifyScreen(1, "staff", bills, { action: "affix_stamp", allowed: true, reason: null }, true);
    expect(html).toContain("収入印紙の貼り付け確認");
    expect(html).not.toContain("ガス料金");
    expect(html).toContain('data-stamp-check="1:staff:1"');
    expect(html).toContain(">200円<");
    expect(html).toContain("data-stampverify-close");
    expect(html).not.toMatch(/。/);
  });

  it("shows the stamp-and-hand-over step after a utility bill payment", () => {
    const st = { action: "hand_over_bills", allowed: true, reason: null };
    const staff = billHandover(1, "staff", 3, st);
    expect(staff).toContain("払込票に収納印を押してお渡しください");
    expect(staff).toContain(`<span class="bill-n">3枚</span>`);
    expect(staff).toContain("お渡し済");
    const customer = billHandover(1, "customer", 3, st);
    expect(customer).toContain("支払いが完了しました");
    expect(customer).toContain(`<span class="bill-n">3枚</span>`);
    expect(customer).toContain("払込票とレシート、お釣りをお受け取りください");
    expect(customer).not.toContain("お渡し済");
  });

  it("shows the open calls only on the self-operated staff screen", () => {
    const call = (kind: string, created_at: number) => ({ id: 1, register: 1, kind, label: kind, created_at, detail: "" });
    const r = {
      staff: { calls: [call("fast_food", 1000), call("tobacco", 2000), call("fast_food", 3000)], bell_down: [] },
      customer: { staff_called: true },
    } as unknown as RegisterView;
    const staff = callScreen(r, 6000);
    expect(staff).toContain("FF＆煙草");
    expect(staff).toContain("5秒経過");
  });

  it("puts no bell-down band on the call screen", () => {
    const r = {
      staff: { calls: [{ id: 1, register: 1, kind: "customer_help", label: "呼び出し", created_at: 1000, detail: "" }], bell_down: ["据え付け端末"] },
      customer: {},
    } as unknown as RegisterView;
    expect(callScreen(r, 6000)).not.toContain("不通");
    expect(callScreen(r, 6000)).not.toContain("banner");
  });

  it("shows no elapsed time for an age-check call", () => {
    const r = {
      staff: { calls: [{ id: 1, register: 1, kind: "age_check", label: "年確", created_at: 1000, detail: "20歳以上" }], bell_down: [] },
      customer: { staff_called: true },
    } as unknown as RegisterView;
    expect(callScreen(r, 6000)).toContain("年確");
    expect(callScreen(r, 6000)).not.toContain("経過");
  });

  it("names the goods, not 現金, for a cash-only call", () => {
    const r = {
      staff: { calls: [{ id: 1, register: 1, kind: "cash_only", label: "現金", created_at: 1000, detail: "チケット" }], bell_down: [] },
      customer: {},
    } as unknown as RegisterView;
    const staff = callScreen(r, 6000);
    expect(staff).toContain("🔔 チケット");
    expect(staff).not.toContain("現金");
  });

  it("marks the item count when the limit is reached", () => {
    expect(regTop(100, 9, "", 10)).not.toContain("上限");
    expect(regTop(100, 10, "", 10)).toContain("上限");
  });

  it("rotates the signage on the attended customer screen's bottom tier", () => {
    expect(signage(0)).toContain(SIGNAGE[0][0]);
    expect(signage(SIGNAGE_MS)).toContain(SIGNAGE[1][0]);
    expect(signage(SIGNAGE_MS * SIGNAGE.length)).toContain(SIGNAGE[0][0]);
    expect(signage(0)).not.toContain("<button");
  });
});
