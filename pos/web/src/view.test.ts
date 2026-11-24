import { describe, expect, it } from "vitest";
import type { RegisterView } from "./types";
import { billHandover, billPayCard, billVerifyScreen, btn, stampVerifyScreen, callScreen, esc, itemCount, menuPaused, regTop, sideOf, signage, SIGNAGE, SIGNAGE_MS, tenderSum, rejectFlash, shownOnScreen } from "./view";
import { trCore } from "./i18n";

describe("view helpers", () => {
  it("keeps the signage out of the language switch", () => {
    expect(signage(0)).toContain("data-i18n-skip");
  });

  it("shows the floor side as closed in the first-wait style while the store menu is open", () => {
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
    const html = btn("会計", { type: "checkout", register: 1, side: "counter" }, { allowed: false, reason: "明細がありません" });
    expect(html).toContain("disabled");
    expect(html).toContain('title="明細がありません"');
  });

  it("routes notices to the side that acted", () => {
    expect(sideOf({ type: "scan", register: 1, side: "floor", code: "" })).toBe("floor");
    expect(sideOf({ type: "insert_cash", register: 1, denomination: 1000, count: 1 })).toBe("floor");
    expect(sideOf({ type: "void_transaction", register: 1 })).toBe("counter");
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
      "counter",
      [
        { line_id: 1, code: "2100000000021", name: "電気料金", amount: 50000, slip_no: "9100 1234 5678 9012", checked: true, stamp: 200, stamped: false },
        { line_id: 2, code: "2100000000043", name: "水道料金", amount: 3000, slip_no: "9100 2222 3333 4444", checked: false, stamp: 0, stamped: false },
        { line_id: 3, code: "2100000000021", name: "電気料金", amount: 50000, slip_no: "9100 5555 6666 7777", checked: false, stamp: 200, stamped: true },
      ],
      { allowed: true, reason: null },
    );
    for (const h of ["事業者", "金額", "バーコード番号", "確認"]) expect(html).toContain(`>${h}<`);
    expect(html).toContain("9100 2222 3333 4444");
    expect(html).not.toContain(">枚数<");
    expect(html.match(/電気料金/g)?.length).toBe(4);
    expect(html).toContain('type="checkbox" checked data-bill-check="1:counter:1"');
    expect(html).toContain('type="checkbox"  data-bill-check="1:counter:3"');
    expect(html).toContain(`<span class="bill-n">3枚</span>`);
    expect(html).toContain("合計 103,000円");
    const pay = billPayCard(3);
    expect(pay).toContain(`公共料金<span class="bill-n">3枚</span>を受け付けます</div>`);
    expect(pay).toContain("現金でお支払いください</p>");
  });

  it("lists only the slips that need a revenue stamp and ticks each one", () => {
    const bills = [
      { line_id: 1, code: "2100000000021", name: "電気料金", amount: 50000, slip_no: "9100 1234 5678 9012", checked: true, stamp: 200, stamped: false },
      { line_id: 2, code: "2100000000043", name: "水道料金", amount: 3000, slip_no: "9100 2222 3333 4444", checked: true, stamp: 0, stamped: false },
    ];
    const html = stampVerifyScreen(1, "counter", bills, { allowed: true, reason: null }, true);
    expect(html).toContain("収入印紙の貼り付け確認");
    expect(html).not.toContain("水道料金");
    expect(html).toContain('data-stamp-check="1:counter:1"');
    expect(html).toContain(">200円<");
    expect(html).toContain("data-stampverify-close");
    expect(html).not.toMatch(/。/);
  });

  it("shows the stamp-and-hand-over step after a utility bill payment", () => {
    const st = { action: "hand_over_bills", allowed: true, reason: null };
    const counter = billHandover(1, "counter", 3, st);
    expect(counter).toContain("払込票に収納印を押してお渡しください");
    expect(counter).toContain(`<span class="bill-n">3枚</span>`);
    expect(counter).toContain("お渡し済");
    const floor = billHandover(1, "floor", 3, st, 140);
    expect(floor).toContain("支払いが完了しました");
    expect(floor).toContain(`<span class="bill-n">3枚</span>`);
    expect(floor).toContain("払込票とレシート、お釣りをお受け取りください");
    // 50,000 円ちょうどならお釣りはないので「お釣り」を外す（B-6）
    expect(billHandover(1, "floor", 1, st, 0)).toContain("払込票とレシートをお受け取りください");
    expect(floor).not.toContain("お渡し済");
  });

  it("shows the open calls only on the self-operated counter-side screen", () => {
    const call = (kind: string, topic: string, created_at: number) => ({ id: 1, register: 1, kind, label: kind, topic, created_at, detail: "" });
    const r = {
      counter: { calls: [call("tobacco", "たばこ", 2000), call("fast_food", "FF", 1000), call("bill", "お支払い", 3000)], bell_down: [] },
      floor: { staff_called: true },
    } as unknown as RegisterView;
    const counter = callScreen(r);
    expect(counter).toContain(">FF<");
    expect(counter).not.toContain("たばこ");
    expect(counter).not.toContain("＆");
    expect(counter).not.toContain("🔔");
    expect(counter).not.toContain("経過");
    expect(counter).toContain("鳴動を止めるにはログインしてください");
  });

  it("puts no bell-down band on the call screen", () => {
    const r = {
      counter: { calls: [{ id: 1, register: 1, kind: "customer_help", label: "呼び出し", created_at: 1000, detail: "" }], bell_down: ["据え付け端末"] },
      floor: {},
    } as unknown as RegisterView;
    expect(callScreen(r)).not.toContain("不通");
    expect(callScreen(r)).not.toContain("banner");
  });

  it("shows no elapsed time for an age-check call", () => {
    const r = {
      counter: { calls: [{ id: 1, register: 1, kind: "age_check", label: "年確", topic: "年確", created_at: 1000, detail: "20歳以上" }], bell_down: [] },
      floor: { staff_called: true },
    } as unknown as RegisterView;
    expect(callScreen(r)).toContain("年確");
    expect(callScreen(r)).not.toContain("経過");
  });

  it("names the goods, not 現金, for a cash-only call", () => {
    const r = {
      counter: { calls: [{ id: 1, register: 1, kind: "cash_only", label: "現金", topic: "チケット", created_at: 1000, detail: "チケット" }], bell_down: [] },
      floor: {},
    } as unknown as RegisterView;
    const counter = callScreen(r);
    expect(counter).toContain(">チケット<");
    expect(counter).not.toContain("現金");
  });

  it("does not mark the item count when the demo item limit is reached (browser alert instead)", () => {
    expect(regTop(100, 100, "")).not.toContain("上限");
  });

  it("rotates the signage on the attended floor-side screen's bottom tier", () => {
    expect(signage(0)).toContain(SIGNAGE[0][0]);
    expect(signage(SIGNAGE_MS)).toContain(SIGNAGE[1][0]);
    expect(signage(SIGNAGE_MS * SIGNAGE.length)).toContain(SIGNAGE[0][0]);
    expect(signage(0)).not.toContain("<button");
  });

  it("splits a rejected reason into a large first line and a small rest", () => {
    expect(rejectFlash("会計を分けてください\nお荷物同士なら、何個でも一緒に会計できます")).toBe(
      '<p class="big">会計を分けてください</p><p class="sub">お荷物同士なら、何個でも一緒に会計できます</p>',
    );
    expect(rejectFlash("サービス商品の個数は変えられません")).toBe('<p class="big">サービス商品の個数は変えられません</p>');
    expect(rejectFlash("Please pay separately\nAny number of parcels can be paid together")).toContain('<p class="sub">Any number of parcels can be paid together</p>');
    // 一行の理由は末尾のかっこ書きを小テキストに、改行のある理由はかっこをそのまま残す
    expect(rejectFlash("お会計中です（登録に戻ってから伝票を読み取ってください）")).toBe('<p class="big">お会計中です</p><p class="sub">登録に戻ってから伝票を読み取ってください</p>');
    expect(rejectFlash("番号が違います\nこの荷物はお渡しできません（お受け取りの荷物は 123）")).toContain('<p class="sub">この荷物はお渡しできません（お受け取りの荷物は 123）</p>');
    expect(rejectFlash("取引中は担当スタッフを切り替えられません\nスタッフ001さんの取引です")).toContain('<p class="sub">スタッフ001さんの取引です</p>');
  });

  it("does not repeat a reason the screen already shows", () => {
    expect(shownOnScreen("ポイントの使い方を選んでください")).toBe(true);
    expect(shownOnScreen("電子マネーが選ばれています（クレジットカードは使えません）")).toBe(true);
    expect(shownOnScreen("あと 120円 入れてください")).toBe(true);
    expect(shownOnScreen("トレカは1回のお会計で3点までです\n3点を超える分はお売りできません")).toBe(false);
    expect(shownOnScreen("お会計中です（登録に戻ってから伝票を読み取ってください）")).toBe(false);
    expect(trCore("トレカは1回のお会計で3点までです\n3点を超える分はお売りできません")).toBe("Trading cards: up to 3 per purchase\nWe cannot sell more than 3");
    expect(trCore("取引中は担当スタッフを切り替えられません\nスタッフ001さんの取引です")).toBe("Staff can't be switched during a transaction\nThis is Staff 001's transaction");
  });

  it("shows total, tendered and change (or a red shortage) while cash goes in (B-4)", () => {
    const ok = tenderSum(640, 1000);
    expect(ok).toContain("お預かり <b>1,000</b>円");
    expect(ok).toContain("お釣り <b>360</b>円");
    const short = tenderSum(640, 500);
    expect(short).toContain('class="tender-cell short">不足 <b>140</b>円');
    // 合計の行は投入前と同じ形のまま、その下に出す
    const band = regTop(1200, 3, "", undefined, "", "", tenderSum(1200, 2000));
    expect(band).toContain('<b class="total">1,200</b>');
    expect(band).toContain("お釣り <b>800</b>円");
  });
});

