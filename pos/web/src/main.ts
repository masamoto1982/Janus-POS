import init, { PosSimulator } from "./wasm/generated/janus_pos.js";
import type { CashOutKind, Command, PaymentMethod, RegisterView, Rounding, Side, Snapshot, TaxKind } from "./types";
import type { GuideKind, RulesTab } from "./view";
import { beepError, beepPress, beepScan, chimeOk, chimePay, setBellRinging, unlockAudio } from "./sound";
import logoUrl from "./logo.jpg";
import { type Lang, langOf, localize, resetLangs, toggleLang, trText } from "./i18n";
import {
  esc,
  FF_TABS,
  ffVerifying,
  stampVerifying,
  expiredClosed,
  ffvInput,
  qtyEntry,
  lineMax,
  flashing,
  type FfUi,
  itemsPanel,
  firstWelcome,
  isWelcome,
  qtyPopup,
  padEntry,
  padPopup,
  PAD,
  padValue,
  padValueKey,
  type PadTarget,
  screenHead,
  storeMenuUsable,
  isoDate,
  scanGuide,
  voidAsk,
  parcelError,
  bellPanel,
  bellSettings,
  ownerPanel,
  shelfPanel,
  walletPanel,
  sideOf,
  sharedScreen,
} from "./view";

declare global {
  interface Window {
    __JANUS_WASM_B64__?: string;
  }
}

const app = document.getElementById("app")!;
let sim: PosSimulator;
let t0 = performance.now();
let snap: Snapshot;
const cache = new WeakMap<Element, string>();
const toastTimers = new Map<string, number>();

function bytesFromB64(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

async function boot(): Promise<void> {
  const b64 = window.__JANUS_WASM_B64__;
  await init(b64 ? { module_or_path: bytesFromB64(b64) } : undefined);
  start();
  setupDrag();
  wireFlip();
  window.addEventListener("pagehide", save);
  window.setInterval(frame, 100);
}

const SAVE_KEY = "janus-pos-state";
const SAVE_EVERY_MS = 1000;
let lastSave = 0;
let discarding = false;

function restore(): PosSimulator | null {
  try {
    const saved = localStorage.getItem(SAVE_KEY);
    return saved ? PosSimulator.restore(saved) : null;
  } catch {
    forget();
    return null;
  }
}

function save(): void {
  if (discarding) return;
  lastSave = performance.now();
  try {
    localStorage.setItem(SAVE_KEY, sim.save());
  } catch {
    return;
  }
}

function forget(): void {
  try {
    localStorage.removeItem(SAVE_KEY);
  } catch {
    return;
  }
}

function start(): void {
  sim = restore() ?? new PosSimulator(Date.now() & 0xffff, 2, 3);
  snap = JSON.parse(sim.snapshot()) as Snapshot;
  t0 = performance.now() - snap.now;
  buildSkeleton();
  app.querySelector("[data-reset]")?.addEventListener("click", () => {
    if (!confirm("保存されている状態を消して、最初からやり直しますか")) return;
    discarding = true;
    forget();
    location.reload();
  });
  wireSwipe();
  wireMenuSwipe();
  wireForceRegister();
  rulesDlg().addEventListener("close", lockRules);
  rulesDlg()
    .querySelector("[data-owner-auth]")
    ?.addEventListener("submit", (e) => {
      e.preventDefault();
      const field = (k: string) => rulesDlg().querySelector<HTMLInputElement>(`[data-owner="${k}"]`);
      const creds = { owner_id: field("id")?.value.trim() ?? "", password: field("pw")?.value ?? "" };
      if (!send({ type: "verify_owner", ...creds })) return;
      ownerAuth = creds;
      field("pw")!.value = "";
      rulesDlg().querySelector<HTMLElement>("[data-owner-auth]")!.hidden = true;
      render();
    });
  render();
}


function buildSkeleton(): void {
  const regs = snap.registers
    .map(
      (r) => `<section class="register" data-register="${r.id}" aria-label="レジ${r.id}">
      <h2>レジ${r.id} <span data-region="mode-${r.id}"></span>
        <label class="login-sel">スタッフ
          <select data-login="${r.id}" aria-label="レジ${r.id}のスタッフログイン">
            <option value="">未ログイン</option>
            ${snap.accounts.map((a) => `<option value="${esc(a.barcode)}">${esc(a.name)}${a.role === "owner" ? "（オーナー）" : ""}</option>`).join("")}
          </select>
        </label>
      </h2>
      <div class="tabs swipe-tabs" role="tablist" aria-label="表示する画面（スワイプでも切り替わります）">
        <button type="button" role="tab" class="tab on" aria-selected="true" data-swipe-to="${r.id}:staff">スタッフ側</button>
        <button type="button" role="tab" class="tab" aria-selected="false" data-swipe-to="${r.id}:customer">お客様側</button>
      </div>
      <div class="screens" data-swipe="${r.id}">
        <div class="screen-wrap" data-wrap="${r.id}:staff">
          <div class="screen-cap">スタッフ側<span class="drop-hint">ここに落とす＝スタッフ側スキャナーで読む</span></div>
          <section class="screen staff-screen" data-drop="${r.id}:staff">
            <div class="screen-head" data-region="head-${r.id}-staff"></div>
            <div class="screen-body"><div class="screen-main" data-region="staff-${r.id}"></div></div>
            <div class="screen-items" data-region="items-${r.id}-staff"></div>
            <div data-region="pop-${r.id}-staff"></div>
          </section>
        </div>
        <div class="screen-wrap" data-wrap="${r.id}:customer">
          <div class="screen-cap">お客様側<span class="drop-hint">ここに落とす＝お客様側スキャナーで読む</span></div>
          <section class="screen customer-screen" data-drop="${r.id}:customer">
            <div class="screen-head" data-region="head-${r.id}-customer"></div>
            <div class="screen-body"><div class="screen-main" data-region="cust-${r.id}"></div></div>
            <div class="screen-items" data-region="items-${r.id}-customer"></div>
            <div data-region="pop-${r.id}-customer"></div>
          </section>
        </div>
      </div>
      <div class="money-io face-staff">
        <div class="money-io-staff" aria-label="レジ${r.id}の自動釣銭機（スタッフ側の画面に付いています）">
          <div class="money-io-boxes" data-cash-drop="${r.id}">
            <button type="button" class="inlet" data-inlet="${r.id}" data-padopen="${r.id}:staff:inlet:" aria-label="レジ${r.id}の入金ボックス（押すとテンキーで金額を入れる）"><span data-region="inlet-${r.id}"></span></button>
            <button type="button" class="outlet" data-outlet="${r.id}" aria-label="レジ${r.id}の出金ボックス（金額をタッチで取り出す）"><span data-region="outlet-${r.id}"></span></button>
          </div>
        </div>
        <div class="money-io-cust" aria-label="レジ${r.id}の決済端末とスキャナ（お客様側の画面に付いています）">
          <div class="money-io-boxes">
            <div data-pay-drop="${r.id}" data-device="terminal"><button type="button" class="terminal" data-terminal="${r.id}" aria-label="レジ${r.id}の決済端末（電子マネー・クレジットカードをかざす）"><span class="placeholder">決済端末</span></button></div>
            <div data-pay-drop="${r.id}" data-device="scanner"><button type="button" class="terminal scanner" data-scanner="${r.id}" aria-label="レジ${r.id}のスキャナ（商品・決済用バーコード・ポイントカードを読む）"><span class="placeholder">スキャナ</span></button></div>
          </div>
        </div>
      </div>
    </section>`,
    )
    .join("");

  app.innerHTML = `
    <header class="top">
      <img class="logo" src="${logoUrl}" width="73" height="73" alt="Janus POS ロゴ" />
      <h1>Janus POS</h1>
      <span class="build-badge" title="ビルド番号">${esc(String(import.meta.env.VITE_BUILD))}</span>
      <button type="button" class="neg small reset-btn" data-reset title="保存されている状態を消して最初からやり直す">リセット</button>
    </header>
    <article class="floor">
      <div class="left">
        <section class="bell-area" aria-label="Bell（呼び鈴）">
          <h2>Bell</h2>
          <div class="bells" data-region="bells"></div>
          <button type="button" class="icon-btn bell-open" data-open-bell aria-haspopup="dialog" aria-label="Bell の設定" title="Bell の設定">⚙</button>
          <div class="toast" id="toast-store"></div>
        </section>
        ${regs}
      </div>
      <div class="right-col">
        <aside class="right" aria-label="Item">
          <h2><span class="tip" tabindex="0" data-tip="お買い物をするには、アイテムをレジエリアにドラッグ＆ドロップします" aria-label="Item：お買い物をするには、アイテムをレジエリアにドラッグ＆ドロップします">Item <span class="info-icon" aria-hidden="true">i</span></span> <button type="button" class="icon-btn rules-open" data-open-rules aria-haspopup="dialog" aria-label="販売規則の設定" title="販売規則の設定">⚙</button></h2>
          <div data-region="shelf"></div>
        </aside>
        <aside class="right wallet" aria-label="Wallet">
          <h2><span class="tip" tabindex="0" data-tip="お金やカードは、ドラッグ＆ドロップで使います：現金は入金ボックス、ポイントカード・クーポンはレジ画面、キャッシュレスは支払方法を選んでから決済端末へ" aria-label="Wallet：お金やカードは、ドラッグ＆ドロップで使います：現金は入金ボックス、ポイントカード・クーポンはレジ画面、キャッシュレスは支払方法を選んでから決済端末へ">Wallet <span class="info-icon" aria-hidden="true">i</span></span></h2>
          <div data-region="wallet"></div>
        </aside>
      </div>
    </article>
    <footer class="bottom">
      <p class="copyright"><a href="https://github.com/masamoto1982" target="_blank" rel="noopener">© 2026 masamoto yamashiro</a></p>
    </footer>
    <dialog class="rules-dlg" id="rules-dlg" aria-labelledby="rules-title">
      <div class="dlg-head"><h2 id="rules-title">販売規則の設定</h2><button class="small ghost" data-close-rules>閉じる</button></div>
      <form class="owner-auth" data-owner-auth>
        <p class="muted">販売規則を変えられるのはオーナーだけです<br>オーナーの従業員番号とパスワードを入れてください<br>閉じると認証は解除されます</p>
        <div class="row wrap">
          <input data-owner="id" placeholder="オーナーの従業員番号" value="000" aria-label="オーナーの従業員番号" />
          <input data-owner="pw" type="password" placeholder="パスワード" autocomplete="off" aria-label="オーナーのパスワード" />
          <button type="submit">認証</button>
        </div>
      </form>
      <div class="toast" id="toast-rules"></div>
      <div data-region="owner"></div>
    </dialog>
    <dialog class="rules-dlg bell-dlg" id="bell-dlg" aria-labelledby="bell-title">
      <div class="dlg-head"><h2 id="bell-title">Bell の設定</h2><button class="small ghost" data-close-bell>閉じる</button></div>
      <p class="muted">可搬性端末とストアコンピューターは、呼び鈴機能を OFF にできます<br>OFF の端末は鳴らず、不通かどうかの判定にも含めません<br>据え付け端末は常に ON です</p>
      <div class="bell-settings" data-region="bell-settings"></div>
    </dialog>
    <dialog class="rules-dlg picker-dlg" id="picker-dlg" aria-label="読ませるものを選ぶ">
      <div class="dlg-head"><button class="small ghost" data-close-picker>閉じる</button></div>
      <div id="picker-body"></div>
    </dialog>
    <div class="drag-ghost" id="ghost" hidden></div>
    ${rampFilter()}`;
}

const RAMP_STEPS: [number, string][] = [
  [0x00, "oklch(0 0 0)"],
  [0x53, "oklch(0.4415 0.11 245)"], // 操作可能部品の青。styles.css の --operable-blue（呼び鈴）と同じ値に保つ
  [0x87, "oklch(0.6246 0.15 245)"],
  [0xae, "oklch(0.7507 0.136 245)"],
  [0xd4, "oklch(0.8704 0.0685 245)"],
  [0xff, "oklch(1 0 0)"],
];

function rampFilter(): string {
  const toRgb = (oklch: string): [number, number, number] => {
    const [L, C, H] = oklch.match(/oklch\(([^)]*)\)/)![1].split(/\s+/).map(Number);
    const a = C * Math.cos((H * Math.PI) / 180);
    const b = C * Math.sin((H * Math.PI) / 180);
    const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
    const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
    const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
    const lin = [
      4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
      -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
      -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
    ];
    return lin.map((v) => {
      const c = Math.min(1, Math.max(0, v));
      return 255 * (c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055);
    }) as [number, number, number];
  };
  const steps = RAMP_STEPS.map(([g, c]) => [g, toRgb(c)] as const);
  const table = (ch: number) =>
    Array.from({ length: 256 }, (_, v) => {
      const i = steps.findIndex(([g]) => g >= v);
      const [g1, c1] = steps[i];
      if (g1 === v || i === 0) return (c1[ch] / 255).toFixed(4);
      const [g0, c0] = steps[i - 1];
      return ((c0[ch] + ((c1[ch] - c0[ch]) * (v - g0)) / (g1 - g0)) / 255).toFixed(4);
    }).join(" ");
  return `<svg width="0" height="0" style="position:absolute" aria-hidden="true" focusable="false">
      <filter id="ramp" color-interpolation-filters="sRGB">
        <feColorMatrix type="saturate" values="0" />
        <feComponentTransfer>
          <feFuncR type="table" tableValues="${table(0)}" />
          <feFuncG type="table" tableValues="${table(1)}" />
          <feFuncB type="table" tableValues="${table(2)}" />
        </feComponentTransfer>
      </filter>
    </svg>`;
}

function setRegion(name: string, raw: string, lang: Lang = "ja"): void {
  const el = app.querySelector(`[data-region="${name}"]`);
  const stamp = `${lang}\u0000${raw}`;
  if (!el || cache.get(el) === stamp) return;
  cache.set(el, stamp);
  const html = localize(raw, lang);
  const values = new Map<string, string>();
  el.querySelectorAll<HTMLInputElement | HTMLSelectElement>("[data-key]").forEach((i) => values.set(i.dataset.key!, i.value));
  const active = document.activeElement as HTMLInputElement | null;
  const activeKey = active && el.contains(active) ? active.dataset.key : undefined;
  const sel = activeKey && "selectionStart" in active! ? active!.selectionStart : null;
  const open = new Set<number>();
  el.querySelectorAll<HTMLDetailsElement>("details[data-keep]").forEach((d, i) => d.open && open.add(i));
  const hadDetails = el.querySelectorAll("details[data-keep]").length > 0;

  el.innerHTML = html;

  el.querySelectorAll<HTMLInputElement | HTMLSelectElement>("[data-key]").forEach((i) => {
    const v = values.get(i.dataset.key!);
    if (v !== undefined) i.value = v;
  });
  if (hadDetails) el.querySelectorAll<HTMLDetailsElement>("details[data-keep]").forEach((d, i) => (d.open = open.has(i)));
  if (activeKey) {
    const next = el.querySelector<HTMLInputElement>(`[data-key="${activeKey}"]`);
    if (next) {
      next.focus();
      if (sel !== null && next.type !== "number") next.setSelectionRange(sel, sel);
    }
  }
}

const FLASH_MS = 900;
const seenQty = new Map<string, number>();
const flashUntil = new Map<string, number>();

const movedRegs = new Set<number>();

function trackFlashes(): void {
  const now = performance.now();
  const present = new Set<string>();
  for (const r of snap.registers) {
    const rows: [number, number][] = [...r.staff.lines.map((l): [number, number] => [l.id, l.qty]), ...r.staff.coupons.map((c): [number, number] => [c.id, 1])];
    for (const [id, qty] of rows) {
      const k = `${r.id}:${id}`;
      present.add(k);
      if (seenQty.get(k) !== qty) {
        seenQty.set(k, qty);
        flashUntil.set(k, now + FLASH_MS);
        movedRegs.add(r.id);
      }
    }
  }
  for (const k of seenQty.keys()) if (!present.has(k)) seenQty.delete(k);
  flashing.clear();
  for (const [k, t] of flashUntil) {
    if (t <= now || !present.has(k)) flashUntil.delete(k);
    else flashing.add(k);
  }
}

const busyRegs = new Set<number>();

function inTransaction(r: RegisterView): boolean {
  const v = r.staff;
  return (
    v.lines.length > 0 ||
    v.coupons.length > 0 ||
    v.point_card !== null ||
    v.returned.length > 0 ||
    v.confiscated.length > 0 ||
    v.ff_collect.length > 0 ||
    r.last_receipt !== null ||
    r.bill_handover !== null ||
    r.parcel_handover !== null ||
    r.ticket_handover.length > 0 ||
    r.receipt_stamp !== null ||
    r.pickup_handover !== null
  );
}

/** レジ前に人がいるか（第一待ち受けの解除）。現物はレジ画面のカメラで人を認識する。
 *  シミュレーターでは、マウスならカーソルがレジ画面に重なっている間、タッチなら最後に触れてから PRESENCE_TOUCH_MS の間 */
const PRESENCE_TOUCH_MS = 30_000;
/** 第一待ち受けへ戻るまでの猶予。取引が始まってから完了するまでは決して戻らず、
 *  人がいなくなった（取引が完了した）あとも、この時間だれも来なければ戻る（棚へ商品を取りに行く・連れを待つ間に切り替わらないように） */
const FIRST_WAIT_AFTER_MS = 30_000;
const lastBusy = new Map<number, number>();
const hovering = new Set<number>();
const touchedAt = new Map<number, number>();
const present = (reg: number): boolean => hovering.has(reg) || Date.now() - (touchedAt.get(reg) ?? -Infinity) < PRESENCE_TOUCH_MS;
const regOf = (t: EventTarget | null): number | null => {
  const el = t instanceof Element ? t.closest<HTMLElement>("[data-swipe]") : null;
  return el ? Number(el.dataset.swipe) : null;
};
document.addEventListener("pointerover", (e) => {
  const reg = regOf(e.target);
  hovering.clear();
  if (reg !== null && e.pointerType === "mouse") hovering.add(reg);
});
document.addEventListener("pointerdown", (e) => {
  const reg = regOf(e.target);
  if (reg !== null && e.pointerType !== "mouse") touchedAt.set(reg, Date.now());
}, true);
document.addEventListener("dragenter", (e) => {
  const reg = regOf(e.target);
  if (reg !== null) touchedAt.set(reg, Date.now());
});
document.documentElement.addEventListener("pointerleave", () => hovering.clear());

function render(): void {
  trackFlashes();
  for (const reg of movedRegs) {
    app.querySelectorAll<HTMLElement>(`[data-drop^="${reg}:"] .screen-main`).forEach((el) => (el.scrollTop = 0));
  }
  movedRegs.clear();
  for (const r of snap.registers) {
    const busy = inTransaction(r);
    if (busyRegs.has(r.id) && !busy) resetLangs(r.id);
    if (busy) busyRegs.add(r.id);
    else busyRegs.delete(r.id);
    const sel = app.querySelector<HTMLSelectElement>(`[data-login="${r.id}"]`);
    if (sel && document.activeElement !== sel) {
      const acc = r.staff.state === "absent" ? undefined : snap.accounts.find((a) => a.name === r.staff.name);
      sel.value = acc?.barcode ?? "";
    }
    for (const side of ["staff", "customer"] as const) {
      const head = app.querySelector<HTMLElement>(`[data-region="head-${r.id}-${side}"]`);
      if (!head) continue;
      head.classList.toggle("sos", r.emergency);
      head.classList.toggle("calling", r.lamp && !r.emergency);
      const lang = langOf(r.id, side);
      app.querySelector(`[data-drop="${r.id}:${side}"]`)?.setAttribute("lang", lang);
      if (r.lamp) {
        head.title = lang === "en" ? trText("呼び出し中") : "呼び出し中";
      } else {
        head.removeAttribute("title");
      }
    }
    // 第一待ち受け：取引のない待ち受けで、レジ前に人がいない（カーソルが重なっていない）状態が FIRST_WAIT_AFTER_MS 続いたとき
    let staffMain = sharedScreen(r, snap, "staff");
    let custMain = sharedScreen(r, snap, "customer");
    const vacant = isWelcome(staffMain) && isWelcome(custMain) && !busy && !r.staff.point_card && !r.customer.point_card;
    if (!vacant || present(r.id)) lastBusy.set(r.id, Date.now());
    const first = vacant && Date.now() - (lastBusy.get(r.id) ?? 0) >= FIRST_WAIT_AFTER_MS;
    if (first) {
      staffMain = firstWelcome(r, "staff");
      custMain = firstWelcome(r, "customer");
    }
    app.querySelector(`[data-swipe="${r.id}"]`)?.classList.toggle("first-wait", first);
    for (const side of ["staff", "customer"] as const) {
      const entry = qtyEntry.get(`${r.id}:${side}`);
      if (entry && (r.staff.phase !== "registering" || !r.staff.lines.some((l) => l.id === entry.id))) qtyEntry.delete(`${r.id}:${side}`);
      const items = itemsPanel(r, snap, ffUi(r.id), side, Date.now(), first);
      const locked = r.active_side !== null && r.active_side !== side;
      const el = app.querySelector(`[data-drop="${r.id}:${side}"]`);
      el?.classList.toggle("inactive", locked);
      const operable = r.active_side !== null ? r.active_side === side : r.mode !== "staff_operated" || side === "staff";
      el?.classList.toggle("operable", operable);
      el?.classList.toggle("mode-staff", r.mode === "staff_operated");
      el?.classList.toggle("mode-self", r.mode === "self_operated");
      el?.classList.toggle("calling", side === "staff" && r.lamp);
      el?.classList.toggle("head-blink", r.lamp || r.emergency);
      el?.classList.toggle("lamp", r.lamp && !r.emergency);
      app.querySelector(`[data-swipe-to="${r.id}:${side}"]`)?.classList.toggle("operable", operable);
      const wrap = el?.parentElement;
      wrap?.classList.toggle("inactive", locked);
      const lang = langOf(r.id, side);
      setRegion(`head-${r.id}-${side}`, screenHead(r, new Date(), side), lang);
      setRegion(`items-${r.id}-${side}`, items, lang);
      const pad = padEntry.get(`${r.id}:${side}`);
      if (pad && pad.target !== "inlet" && !el?.querySelector(`[data-padopen="${r.id}:${side}:${pad.target}:${pad.line ?? ""}"]`)) padEntry.delete(`${r.id}:${side}`);
      if (side === "staff" && !["refund", "emergency", "expense"].includes(r.menu?.screen ?? "")) {
        padValue.delete(padValueKey(r.id, "mamt"));
        padValue.delete(padValueKey(r.id, "mpw"));
      }
      // 荷物の長さ・重さ・郵便番号は受付中の荷物のものだけ残す。明細の番号は取引ごとに振り直すので、
      // 残しておくと次の取引の荷物に前の荷物の値が入ってしまう
      if (side === "staff") {
        const pending = new Set(r.staff.lines.filter((l) => l.parcel && l.kind === "service" && !l.done).map((l) => String(l.id)));
        for (const key of [...padValue.keys()]) {
          const [reg, target, line] = key.split(":");
          if (reg === String(r.id) && ["pcm", "pkg", "post"].includes(target) && !pending.has(line)) padValue.delete(key);
        }
      }
      // ポイントの使い方の画面を離れたら、指定しかけたポイント数を捨てる
      if (r.customer.method !== "points" || r.prepaid) padValue.delete(padValueKey(r.id, "ppt"));
      setRegion(`pop-${r.id}-${side}`, qtyPopup(r, side) || padPopup(r, side), lang);
      el?.classList.toggle("no-items", !items);
    }
    const face: Side = r.menu ? "staff" : r.mode === "staff_operated" ? "staff" : "customer";
    if (face !== activeShown.get(r.id)) {
      activeShown.set(r.id, face);
      if (isMobile()) swipeTo(r.id, face);
    }
    const idle = r.idle_revert_in;
    setRegion(
      `mode-${r.id}`,
      `${
        idle !== null
          ? `<span class="idle" title="5分間操作がないと、登録中の取引も中止してセルフ操作モードに戻ります">無操作であと ${Math.floor(idle / 60000)}:${String(Math.floor((idle % 60000) / 1000)).padStart(2, "0")} で${
              r.idle_voids ? (r.mode === "self_operated" ? "取引を中止します" : "取引を中止してセルフ操作モードに戻ります") : "セルフ操作モードに戻ります"
            }</span>`
          : ""
      }`,
    );
    setRegion(`staff-${r.id}`, staffMain, langOf(r.id, "staff"));
    setRegion(`cust-${r.id}`, custMain, langOf(r.id, "customer"));
    setRegion(
      `outlet-${r.id}`,
      r.outlet ? `<b title="${r.outlet_detail.map(([d, n]) => `${d.toLocaleString()}円×${n}`).join(" ")}">${r.outlet.toLocaleString()}円</b>` : `<span class="placeholder">出金</span>`,
    );
    app.querySelector(`[data-outlet="${r.id}"]`)?.classList.toggle("blink", r.outlet > 0);
    const inlet = app.querySelector<HTMLElement>(`[data-inlet="${r.id}"]`);
    const terminal = app.querySelector<HTMLElement>(`[data-terminal="${r.id}"]`);
    const paying = !r.menu && r.staff.phase === "paying" && r.staff.method !== null && r.emoney_short === null && r.emoney_partial === null;
    const emoney = r.menu ? null : r.emoney;
    const partial = !r.menu && r.emoney_partial !== null && r.emoney_partial_ok;
    const cashPay = paying && r.staff.method === "cash";
    const charging = !!emoney && emoney.kind === "charge";
    const tapping = !!emoney && emoney.kind !== "menu" && !(charging && !emoney.inserted);
    if (inlet) {
      inlet.classList.toggle("blink", cashPay || (charging && !emoney!.inserted));
      const hint = cashPay || (charging && !emoney!.inserted) ? (charging ? "紙幣を入れる" : "現金を入れる") : "入金";
      const shown = emoney ? emoney.inserted : !r.menu ? r.staff.inserted : 0;
      setRegion(`inlet-${r.id}`, shown ? `<b>${shown.toLocaleString()}円</b>` : `<span class="placeholder">${esc(hint)}</span>`);
    }
    if (terminal) {
      // 決済端末は、電子マネー・クレジットカードを待っているあいだだけ青白くゆったり明滅する（スキャナは待機中は光らない）。
      // かざした直後に決済が終わっても、光が広がり終わるまでは待ち受けの見た目のまま
      const reader = (paying && READER_METHODS.has(r.staff.method!)) || partial || tapping || terminal.classList.contains("fx-tap");
      terminal.classList.toggle("reader", reader);
    }
  }
  setRegion("bells", bellPanel(snap));
  setRegion("bell-settings", bellSettings(snap));
  setRegion("owner", ownerAuth ? ownerPanel(snap, rulesTab) : "");
  setRegion("shelf", shelfPanel(snap));
  applyFlips(app.querySelector('[data-region="shelf"]'));
  setRegion("wallet", walletPanel(snap.emoney_balance));
}


const ffTabs = new Map<number, FfUi>();
const ffUi = (reg: number): FfUi => ffTabs.get(reg) ?? { tab: FF_TABS[0][0], page: 0 };

function frame(): void {
  sim.tick(performance.now() - t0);
  snap = JSON.parse(sim.snapshot()) as Snapshot;
  setBellRinging(
    snap.bells.some((m) => m.buzzing),
    Math.max(0, ...snap.registers.map((r) => r.ring_volume)),
  );
  render();
  if (performance.now() - lastSave >= SAVE_EVERY_MS) save();
  app.querySelectorAll<HTMLElement>("[data-hold]").forEach((b) => b.classList.toggle("pressing", b.dataset.hold === held?.cmd));
}

const activeShown = new Map<number, Side>();

let held: { cmd: string; register: number; pointer: number | null } | null = null;

function press(b: HTMLElement, pointer: number | null): void {
  if (held || (b as HTMLButtonElement).disabled) return;
  const cmd = b.dataset.hold!;
  const c = JSON.parse(cmd) as Command;
  if (send(c)) held = { cmd, register: "register" in c ? c.register : 0, pointer };
  frame();
}

function release(): void {
  if (!held) return;
  const r = held.register;
  held = null;
  send({ type: "release_call", register: r });
}

function toast(where: string, text: string, error: boolean): void {
  const el = document.getElementById(where);
  if (!el) return;
  el.textContent = text;
  el.className = `toast show ${error ? "error" : ""}`;
  window.clearTimeout(toastTimers.get(where));
  toastTimers.set(
    where,
    window.setTimeout(() => (el.className = "toast"), error ? 4000 : 2500),
  );
}

/** テンキーの「決定」でそのまま送る入力欄（画面側に決定・判定ボタンを置かない）。
 *  テンキーの「決定」が画面の決定ボタンとほぼ同じ位置に重なり、二度押しが要るように見えたため（動作試験 第2回） */
const SUBMIT: Partial<Record<PadTarget, (register: number, side: Side, digits: string, line?: number) => Command | null>> = {
  birth: (register, side, birth) => ({ type: "check_id", register, side, birth, today: isoDate(new Date()) }),
  pcm: (register, _side, digits, line) => ({ type: "parcel_length", register, line_id: line!, sum_cm: Number(digits) }),
  pkg: (register, _side, digits, line) => {
    const sum = snap.registers.find((r) => r.id === register)?.staff.lines.find((l) => l.id === line)?.parcel?.sum_cm;
    return sum === undefined || sum === null ? null : { type: "parcel_size", register, line_id: line!, sum_cm: sum, weight_g: Math.round(Number(digits) * 1000) };
  },
  post: (register, _side, postal, line) => ({ type: "parcel_postal", register, line_id: line!, postal }),
  ppt: (register, side, digits) => ({ type: "use_points", register, side, how: "amount", points: Number(digits) }),
};

function padKey(key: string, k: string): void {
  const e = padEntry.get(key);
  if (!e) return;
  const d = PAD[e.target];
  const register = Number(key.split(":")[0]);
  if (k !== "ok") e.error = undefined;
  if (k === "cancel") padEntry.delete(key);
  else if (k === "back") e.digits = e.digits.slice(0, -1);
  else if (k === "ok") {
    const value = Number(e.digits || 0);
    const side = key.split(":")[1];
    if (e.target === "inlet") {
      const r = snap.registers.find((x) => x.id === register);
      if (r?.menu) {
        if (value <= 0 || !send({ type: "insert", register, amount: value })) return;
      } else {
        if (value > 0 && !send({ type: "insert", register, amount: value })) return;
        send({ type: "finish_insert", register });
      }
    } else if (e.target === "tkh") {
      if (e.digits === "") return;
      send({ type: "hand_tickets", register, side: "staff", handed: value });
    } else if (e.target === "tkr") {
      if (e.digits === "") return;
      send({ type: "receive_tickets", register, side: "customer", received: value });
    } else if (e.target === "ffv") {
      if (e.digits === "") return;
      const ffKey = `${register}:${side}:${e.line}`;
      ffvInput.set(ffKey, String(value));
      send({ type: "check_ff", register, side: side as Side, line_id: e.line!, handed: value });
    } else if (e.target in SUBMIT) {
      // 入力欄ひとつの画面：テンキーの「決定」でそのまま送る。受け付けられなければ理由を出してテンキーを開いたままにする
      if (e.digits === "") return;
      const cmd = SUBMIT[e.target]!(register, side as Side, e.digits, e.line);
      if (!cmd || !send(cmd)) {
        e.error = lastError ?? "受け付けられません";
        frame();
        return;
      }
      padValue.delete(padValueKey(register, e.target, e.line));
    } else {
      if (e.digits === "" || (!d.code && value <= 0)) return;
      padValue.set(padValueKey(register, e.target, e.line), d.code ? e.digits : String(value));
    }
    padEntry.delete(key);
  } else if (k === ".") {
    if (d.decimal && !e.digits.includes(".") && e.digits.replace(".", "").length < d.max) e.digits = (e.digits || "0") + ".";
  } else if (/^[0-9]$/.test(k) && e.digits.replace(".", "").length < d.max) {
    const next = e.digits + k;
    if (/\.\d{2}$/.test(next)) return;
    e.digits = next.includes(".") || d.code ? next : next.replace(/^0+/, "");
  }
  frame();
}

function qtyKey(key: string, k: string): void {
  const e = qtyEntry.get(key);
  if (!e) return;
  if (k === "cancel") qtyEntry.delete(key);
  else if (k === "back") e.digits = e.digits.slice(0, -1);
  else if (k === "ok") {
    if (!e.digits || Number(e.digits) <= 0) return;
    const [register, side] = key.split(":");
    send({ type: "set_qty", register: Number(register), side: side as Side, line_id: e.id, qty: Number(e.digits) });
    qtyEntry.delete(key);
  } else if (/^[0-9]$/.test(k) && e.digits.length < 2) {
    // 販売個数の上限を超える数はテンキーでも打てない
    const [register, side] = key.split(":");
    const r = snap.registers.find((x) => x.id === Number(register));
    const lines = side === "staff" ? r?.staff.lines ?? [] : r?.customer.lines ?? [];
    const l = lines.find((x) => x.id === e.id);
    const max = l ? lineMax(l, lines) : undefined;
    const next = (e.digits + k).replace(/^0+/, "");
    if (max === undefined || Number(next) <= max) e.digits = next;
  }
  frame();
}

/** スタッフバーコードの頭（core catalog::STAFF_PREFIX と同じ） */
const STAFF_BARCODE_PREFIX = "ST";

/** 読み取った瞬間の演出。読んだ機器のボックスに出す：
 *  スキャナ＝商品（レジ画面に商品を入れるのも同じ）・決済用バーコード・ポイントカード→赤帯が左から右へ、
 *  決済端末＝電子マネー・クレジットカード→中心から光が広がる（決済端末が待ち受けているときだけ） */
function deviceFx(cmd: Command): (() => void) | null {
  if (cmd.type !== "scan" && cmd.type !== "tap_emoney") return null;
  // スタッフバーコード（ログイン・担当者の切り替え）では赤帯を出さない。
  // 経緯（user 2026-11-12）：シミュレーターではスタッフの選択をスタッフバーコードの読み取り（scan コマンド）として送るので、
  // 演出を出すこと自体には理由がある。ただ、リポジトリのオーナーの目にも不思議に映ったため、便宜的に対象外にした。
  // 商品・決済用バーコード・ポイントカードの読み取りは従来どおり赤帯を出す
  if (cmd.type === "scan" && cmd.code.startsWith(STAFF_BARCODE_PREFIX)) return null;
  const scanned = cmd.type === "scan" || cmd.method === "qr" || cmd.method === "points";
  const el = app.querySelector<HTMLElement>(scanned ? `[data-scanner="${cmd.register}"]` : `[data-terminal="${cmd.register}"]`);
  if (!el || (!scanned && !el.classList.contains("reader"))) return null;
  const cls = scanned ? "fx-scan" : "fx-tap";
  return () => {
    el.classList.remove("fx-scan", "fx-tap");
    void el.offsetWidth; // 続けて読んだときも最初から再生する
    el.classList.add(cls);
    window.clearTimeout(Number(el.dataset.fxTimer));
    el.dataset.fxTimer = String(window.setTimeout(() => el.classList.remove(cls), 1000));
  };
}

/** 直前の send が受け付けられなかった理由 */
let lastError: string | undefined;

function send(cmd: Command): boolean {
  const fx = deviceFx(cmd);
  sim.tick(performance.now() - t0);
  const res = JSON.parse(sim.dispatch(JSON.stringify(cmd))) as { ok: boolean; message?: string; error?: string };
  lastError = res.ok ? undefined : res.error;
  const where = RULE_CMDS.includes(cmd.type) ? "toast-rules" : sideOf(cmd) === "store" ? "toast-store" : null;
  if (where) {
    if (res.ok) {
      if (res.message) toast(where, res.message, false);
    } else toast(where, res.error ?? "操作できません", true);
  }
  if (!res.ok) {
    if (res.error?.includes("買い物点数の上限")) setTimeout(() => alert(res.error), 0);
  }
  if (cmd.type.startsWith("parcel_")) {
    if (res.ok) parcelError.delete("register" in cmd ? cmd.register : 0);
    else parcelError.set("register" in cmd ? cmd.register : 0, res.error ?? "受け付けられません");
  }
  // 販売期限切れの品は登録はされるが、一緒に決済できない商品と同じ異常音（ブブッ）を鳴らす
  const expired = cmd.type === "scan" && !!res.message?.includes("販売期限が切れています");
  // 決済端末（電子マネー・クレジットカード）で払えたら「シャーン！」、スキャナで読んだものはピッ
  const paidAtTerminal = cmd.type === "tap_emoney" && cmd.method !== "qr" && cmd.method !== "points";
  // 電子マネーの残高不足は払えていないので決済音を鳴らさず異常音（ブブッ）：同じ音だと支払いが済んだと勘違いして退店してしまう
  const emoneyShort = cmd.type === "tap_emoney" && !!res.message?.startsWith("電子マネーの残高が足りません");
  if (["scan", "register_ff", "tap_emoney"].includes(cmd.type)) (res.ok && !expired && !emoneyShort ? (paidAtTerminal ? chimePay : beepScan) : beepError)();
  // 照合イベント（年確・FFのお渡し）が済んだら「ピンポン」
  if (res.ok && ((cmd.type === "check_id" && !!res.message?.includes("年齢確認が済みました")) || (cmd.type === "check_ff" && !!res.message?.includes("すべてのFFのお渡しを確認しました")))) chimeOk();
  frame();
  fx?.();
  if (res.ok) save();
  return res.ok;
}

const hoverTip = document.createElement("div");
hoverTip.className = "hover-tip";
hoverTip.setAttribute("role", "tooltip");
hoverTip.hidden = true;
document.body.append(hoverTip);
let hintFor: HTMLElement | null = null;
let hintTimer = 0;
const hideHint = (): void => {
  window.clearTimeout(hintTimer);
  hintFor = null;
  hoverTip.hidden = true;
};
document.addEventListener("pointerover", (e) => {
  if (e.pointerType === "touch") return;
  const el = (e.target as HTMLElement).closest<HTMLElement>("[data-hint]");
  if (!el || el === hintFor) return;
  hideHint();
  hintFor = el;
  hintTimer = window.setTimeout(() => {
    if (!el.isConnected || document.body.classList.contains("dragging")) return;
    hoverTip.textContent = el.dataset.hint ?? "";
    hoverTip.hidden = false;
    const r = el.getBoundingClientRect();
    const w = hoverTip.offsetWidth;
    const h = hoverTip.offsetHeight;
    const left = Math.max(8, Math.min(r.left, window.innerWidth - w - 8));
    const below = r.bottom + 6 + h <= window.innerHeight - 8;
    hoverTip.style.left = `${left}px`;
    hoverTip.style.top = `${below ? r.bottom + 6 : r.top - h - 6}px`;
  }, 350);
});
document.addEventListener("pointerout", (e) => {
  if (!hintFor) return;
  const to = e.relatedTarget as Node | null;
  if (!to || !hintFor.contains(to)) hideHint();
});
document.addEventListener("pointerdown", hideHint, true);
window.addEventListener("scroll", hideHint, true);

let scope: ParentNode = app;
const val = (key: string): string => scope.querySelector<HTMLInputElement>(`[data-key="${key}"]`)?.value.trim() ?? "";

function form(b: HTMLElement): void {
  const reg = Number(b.dataset.reg);
  scope = b.closest("[data-drop]") ?? app;
  switch (b.dataset.form) {
    case "service":
      send({ type: "complete_service", register: reg, line_id: Number(b.dataset.line), amount: Number(val(`svc-${reg}-${b.dataset.line}`) || 0) });
      break;
    case "cashout":
      if (
        send({
          type: "cash_out",
          register: reg,
          kind: b.dataset.kind as CashOutKind,
          amount: Number(padValue.get(padValueKey(reg, "mamt")) ?? 0),
          password: padValue.get(padValueKey(reg, "mpw")) ?? "",
        })
      )
        padValue.delete(padValueKey(reg, "mamt"));
      padValue.delete(padValueKey(reg, "mpw"));
      frame();
      break;
  }
}

app.addEventListener("input", (e) => {
  const el = e.target as HTMLInputElement;
  if (el.dataset.billCheck) {
    const [reg, side, line] = el.dataset.billCheck.split(":");
    send({ type: "check_bill", register: Number(reg), side: side as Side, line_id: Number(line), checked: el.checked });
    return;
  }
  if (el.dataset.receiptStamp) {
    const [reg, side] = el.dataset.receiptStamp.split(":");
    if (el.checked) send({ type: "affix_receipt_stamp", register: Number(reg), side: side as Side });
    return;
  }
  if (el.dataset.stampCheck) {
    const [reg, side, line] = el.dataset.stampCheck.split(":");
    send({ type: "affix_stamp", register: Number(reg), side: side as Side, line_id: Number(line), affixed: el.checked });
    return;
  }
});

app.addEventListener("change", (e) => {
  const sel = e.target as HTMLSelectElement;
  if (sel.dataset.bellOn) {
    const box = sel as unknown as HTMLInputElement;
    if (!send({ type: "set_bell_on", bell: Number(box.dataset.bellOn), on: box.checked })) box.checked = !box.checked;
    return;
  }
  if (sel.hasAttribute("data-bells-offline")) {
    const box = sel as unknown as HTMLInputElement;
    snap.bells.forEach((_, i) => send({ type: "set_bell_offline", bell: i, offline: box.checked }));
    return;
  }
  if (sel.dataset.ffcheck) {
    const box = sel as unknown as HTMLInputElement;
    const [reg, side] = box.dataset.ffreg!.split(":");
    if (!box.checked) return;
    const line = Number(box.dataset.ffcheck);
    ffvInput.set(`${reg}:${side}:${line}`, "1");
    if (!send({ type: "check_ff", register: Number(reg), side: side as Side, line_id: line, handed: 1 })) box.checked = false;
    return;
  }
  if (sel.dataset.taxRate && ownerAuth) {
    const rate = Number((sel as unknown as HTMLInputElement).value);
    const ok = Number.isFinite(rate) && send({ type: "set_tax_rate", ...ownerAuth, class_id: sel.dataset.taxRate, rate_bp: Math.round(rate * 100) });
    if (!ok) (sel as unknown as HTMLInputElement).value = (sel as unknown as HTMLInputElement).defaultValue;
    return;
  }
  if (sel.dataset.taxClass && ownerAuth) {
    const ok = send({ type: "set_product_tax", ...ownerAuth, code: sel.dataset.taxClass, class_id: sel.value });
    if (!ok) for (const o of sel.options) o.selected = o.defaultSelected;
    return;
  }
  if (sel.dataset.age && ownerAuth) {
    const ok = send({
      type: "set_product_age",
      ...ownerAuth,
      code: sel.dataset.age,
      min_age: sel.value ? Number(sel.value) : null,
    });
    if (!ok) for (const o of sel.options) o.selected = o.defaultSelected;
    return;
  }
  if (sel.dataset.limit && ownerAuth) {
    const ok = send({
      type: "set_sale_limit",
      ...ownerAuth,
      code: sel.dataset.limit,
      limit: sel.value ? Number(sel.value) : null,
    });
    if (!ok) for (const o of sel.options) o.selected = o.defaultSelected;
    return;
  }
  if (!sel.dataset.login) return;
  const register = Number(sel.dataset.login);
  if (sel.value) send({ type: "scan", register, side: "staff", code: sel.value });
  else send({ type: "end_staff_mode", register });
  sel.blur();
});

const rulesDlg = (): HTMLDialogElement => document.getElementById("rules-dlg") as HTMLDialogElement;

const RULE_CMDS = ["verify_owner", "set_product_age", "set_sale_limit", "set_tax_rate", "add_tax_class", "set_product_tax", "set_invoice"];
let ownerAuth: { owner_id: string; password: string } | null = null;
let rulesTab: RulesTab = "tax";

function lockRules(): void {
  ownerAuth = null;
  rulesTab = "tax";
  const pw = rulesDlg().querySelector<HTMLInputElement>(`[data-owner="pw"]`);
  if (pw) pw.value = "";
  const form = rulesDlg().querySelector<HTMLElement>("[data-owner-auth]");
  if (form) form.hidden = false;
  render();
}

document.addEventListener("pointerdown", unlockAudio, { capture: true });
document.addEventListener("keydown", unlockAudio, { capture: true });
document.addEventListener(
  "pointerdown",
  (e) => {
    const b = (e.target as Element | null)?.closest<HTMLButtonElement>("button");
    if (b && !b.disabled && !b.matches(".item.off")) beepPress();
  },
  { capture: true },
);
document.addEventListener("keydown", (e) => {
  if ((!qtyEntry.size && !padEntry.size) || e.isComposing || (e.target as HTMLElement).closest?.("input, select, textarea")) return;
  const k = /^[0-9.]$/.test(e.key) ? e.key : e.key === "Backspace" ? "back" : e.key === "Enter" ? "ok" : e.key === "Escape" ? "cancel" : null;
  if (!k) return;
  e.preventDefault();
  if (qtyEntry.size) qtyKey([...qtyEntry.keys()].at(-1)!, k);
  else padKey([...padEntry.keys()].at(-1)!, k);
});
document.addEventListener("touchend", unlockAudio, { capture: true });

app.addEventListener("click", (e) => {
  const backdrop = (e.target as HTMLElement).dataset?.qtypadBackdrop;
  if (backdrop) return qtyKey(backdrop, "cancel");
  const padBackdrop = (e.target as HTMLElement).dataset?.padBackdrop;
  if (padBackdrop) return padKey(padBackdrop, "cancel");
  const b = (e.target as Element).closest("button");
  if (!b || b.disabled) return;
  if (b.hasAttribute("data-open-rules")) return rulesDlg().showModal();
  if (b.hasAttribute("data-close-rules")) return rulesDlg().close();
  if (b.dataset.rulesTab) {
    rulesTab = b.dataset.rulesTab as RulesTab;
    return render();
  }
  if (b.hasAttribute("data-add-tax") && ownerAuth) {
    const field = (k: string) => rulesDlg().querySelector<HTMLInputElement>(`[data-new-tax="${k}"]`)!;
    const rate = Number(field("rate").value || 0);
    if (
      send({ type: "add_tax_class", ...ownerAuth, name: field("name").value, kind: field("kind").value as TaxKind, rate_bp: Math.round(rate * 100) })
    ) {
      field("name").value = "";
      field("rate").value = "";
    }
    return;
  }
  if (b.hasAttribute("data-save-invoice") && ownerAuth) {
    const field = (k: string) => rulesDlg().querySelector<HTMLInputElement>(`[data-invoice="${k}"]`)!;
    send({ type: "set_invoice", ...ownerAuth, invoice_no: field("no").value, rounding: field("rounding").value as Rounding });
    return;
  }
  if (b.hasAttribute("data-open-bell")) return (document.getElementById("bell-dlg") as HTMLDialogElement).showModal();
  if (b.hasAttribute("data-close-bell")) return (document.getElementById("bell-dlg") as HTMLDialogElement).close();
  if (b.dataset.fftab) {
    const [reg, tab] = b.dataset.fftab.split(":");
    ffTabs.set(Number(reg), { tab, page: 0 });
    return frame();
  }
  if (b.dataset.ffpage) {
    const [reg, d] = b.dataset.ffpage.split(":").map(Number);
    const u = ffUi(reg);
    ffTabs.set(reg, { tab: u.tab, page: Math.max(0, u.page + d) });
    return frame();
  }
  if (b.dataset.outlet) {
    send({ type: "take_outlet", register: Number(b.dataset.outlet) });
    return;
  }
  if (b.dataset.voidAsk) {
    const [reg, side] = b.dataset.voidAsk.split(":");
    const r = Number(reg);
    voidAsk.set(r, side as "staff" | "customer");
    return frame();
  }
  if (b.dataset.voidCancel) {
    voidAsk.delete(Number(b.dataset.voidCancel));
    return frame();
  }
  if (b.dataset.lang) {
    const [reg, side] = b.dataset.lang.split(":");
    toggleLang(Number(reg), side);
    return frame();
  }
  if (b.dataset.guide) {
    const [reg, side, kind] = b.dataset.guide.split(":");
    const key = `${reg}:${side}`;
    if (scanGuide.get(key) === kind) scanGuide.delete(key);
    else scanGuide.set(key, kind as GuideKind);
    frame();
    return;
  }
  if (b.dataset.ffverify) {
    ffVerifying.add(b.dataset.ffverify);
    frame();
    return;
  }
  if (b.dataset.expiredview) {
    expiredClosed.delete(b.dataset.expiredview);
    frame();
    return;
  }
  if (b.dataset.expiredClose) {
    expiredClosed.set(b.dataset.expiredClose, b.dataset.expiredSig ?? "");
    frame();
    return;
  }
  if (b.dataset.stampverify) {
    stampVerifying.add(b.dataset.stampverify);
    frame();
    return;
  }
  if (b.dataset.stampverifyClose) {
    stampVerifying.delete(b.dataset.stampverifyClose);
    frame();
    return;
  }
  if (b.dataset.ffverifyClose) {
    ffVerifying.delete(b.dataset.ffverifyClose);
    for (const k of [...ffvInput.keys()]) if (k.startsWith(`${b.dataset.ffverifyClose}:`)) ffvInput.delete(k);
    frame();
    return;
  }
  if (b.dataset.qtyentry) {
    const [reg, side, id] = b.dataset.qtyentry.split(":");
    const key = `${reg}:${side}`;
    if (qtyEntry.get(key)?.id === Number(id)) qtyEntry.delete(key);
    else {
      padEntry.delete(key);
      qtyEntry.set(key, { id: Number(id), digits: "" });
    }
    frame();
    return;
  }
  if (b.dataset.padopen) {
    const [reg, s0, target, line] = b.dataset.padopen.split(":");
    const side = s0;
    const key = `${reg}:${side}`;
    if (padEntry.get(key)?.target === target) padEntry.delete(key);
    else {
      qtyEntry.delete(key);
      padEntry.set(key, { target: target as PadTarget, line: line ? Number(line) : undefined, digits: "" });
    }
    frame();
    return;
  }
  if (b.dataset.pad) {
    const [reg, side, k] = b.dataset.pad.split(":");
    padKey(`${reg}:${side}`, k);
    return;
  }
  if (b.dataset.qtypad) {
    const [reg, side, k] = b.dataset.qtypad.split(":");
    qtyKey(`${reg}:${side}`, k);
    return;
  }
  if (b.dataset.cmd) send(JSON.parse(b.dataset.cmd) as Command);
  else if (b.dataset.form) form(b);
});

/** 決済端末として読む支払方法（バーコード決済・ポイントはスキャナで読む） */
const READER_METHODS = new Set(["emoney", "credit"]);

function setupDrag(): void {
  const ghost = document.getElementById("ghost")!;
  let drag: { code?: string; wallet?: string; flip: boolean; label: string; x: number; y: number; moved: boolean; id: number } | null = null;

  const target = (x: number, y: number): Element | null => document.elementFromPoint(x, y)?.closest("[data-drop], [data-cash-drop], [data-pay-drop]") ?? null;
  const end = () => {
    drag = null;
    ghost.hidden = true;
    document.body.classList.remove("dragging", "dragging-cash", "dragging-tap");
  };

  app.addEventListener("pointerdown", (e) => {
    const b = (e.target as Element).closest<HTMLElement>("[data-code], [data-wallet]");
    if (!b || e.button !== 0 || b.closest("#picker-dlg")) return;
    e.preventDefault();
    drag = { code: b.dataset.code, wallet: b.dataset.wallet, flip: !!b.dataset.flip, label: b.querySelector(".item-name")?.textContent ?? "", x: e.clientX, y: e.clientY, moved: false, id: e.pointerId };
  });

  window.addEventListener("pointermove", (e) => {
    if (!drag || e.pointerId !== drag.id) return;
    if (!drag.moved && Math.hypot(e.clientX - drag.x, e.clientY - drag.y) < 5) return;
    if (!drag.moved) {
      drag.moved = true;
      ghost.textContent = drag.label;
      ghost.hidden = false;
      document.body.classList.add("dragging");
      // ドラッグ中は、それを受け取る機器の枠だけを強調する：現金＝スタッフ側の入金ボックス、
      // 電子マネー・クレジットカード＝お客様側の決済端末（バーコード決済はスキャナで、赤い枠のまま）
      document.body.classList.toggle("dragging-cash", !!drag.wallet?.startsWith("cash:"));
      document.body.classList.toggle("dragging-tap", drag.wallet === "pay:emoney" || drag.wallet === "pay:credit");
    }
    ghost.style.transform = `translate(${e.clientX + 8}px, ${e.clientY + 8}px)`;
  });

  window.addEventListener("pointerup", (e) => {
    if (!drag || e.pointerId !== drag.id) return;
    const d = drag;
    const t = target(e.clientX, e.clientY);
    end();
    if (d.moved) dragMoved = true;
    if (!d.moved) {
      // 裏のある商品ボタンは、クリックで裏返す（click で受ける）。ドラッグでしか登録できないことの案内は、裏のないボタンだけ
      if (d.flip) return;
      toast("toast-store", "ドラッグして、レジのスタッフ側の画面・お客様画面（現金は入金ボックス、キャッシュレスは決済端末にも）に落としてください", false);
      return;
    }
    if (!t) return;
    const el = t as HTMLElement;
    const [reg, side] = el.dataset.drop ? el.dataset.drop.split(":") : [el.dataset.cashDrop ?? el.dataset.payDrop!, el.dataset.cashDrop ? "staff" : "customer"];
    deliver({ register: Number(reg), side: side as Side, device: el.dataset.device as Device | undefined, cashBox: !!el.dataset.cashDrop }, d.code, d.wallet);
  });

  window.addEventListener("pointercancel", end);

  app.addEventListener("pointerdown", (e) => {
    const b = (e.target as Element).closest<HTMLElement>("[data-hold]");
    if (!b || e.button !== 0) return;
    e.preventDefault();
    press(b, e.pointerId);
  });
  window.addEventListener("pointermove", (e) => {
    if (!held || e.pointerId !== held.pointer) return;
    const over = document.elementFromPoint(e.clientX, e.clientY)?.closest<HTMLElement>("[data-hold]");
    if (over?.dataset.hold !== held.cmd) release();
  });
  for (const ev of ["pointerup", "pointercancel"] as const) {
    window.addEventListener(ev, (e) => {
      if (held && e.pointerId === held.pointer) release();
    });
  }
  window.addEventListener("blur", release);
  document.addEventListener("visibilitychange", release);
  app.addEventListener("contextmenu", (e) => {
    if ((e.target as Element).closest("[data-hold]")) e.preventDefault();
  });
  app.addEventListener("keydown", (e) => {
    const b = (e.target as Element).closest<HTMLElement>("[data-hold]");
    if (!b || e.repeat || (e.key !== " " && e.key !== "Enter")) return;
    e.preventDefault();
    press(b, null);
  });
  app.addEventListener("keyup", (e) => {
    if (held && held.pointer === null && (e.key === " " || e.key === "Enter")) release();
  });
}

/** 裏返している Item の商品ボタン（商品コード）。描き直しても裏返したまま */
const flipped = new Set<string>();
const FLIP_MS = 280;
let dragMoved = false;

function applyFlips(root: ParentNode | null): void {
  root?.querySelectorAll<HTMLElement>("[data-flip]").forEach((b) => b.classList.toggle("flipped", flipped.has(b.dataset.flip!)));
}

/** カードを裏返すように、表（商品名・価格）と裏（それ以外）を入れ替える。半分回ったところで面を替える */
function flip(b: HTMLElement): void {
  const code = b.dataset.flip!;
  if (flipped.has(code)) flipped.delete(code);
  else flipped.add(code);
  const swap = () => b.classList.toggle("flipped", flipped.has(code));
  if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) { swap(); return; }
  b.classList.remove("flipping");
  void b.offsetWidth;
  b.classList.add("flipping");
  window.setTimeout(swap, FLIP_MS / 2);
  window.setTimeout(() => b.classList.remove("flipping"), FLIP_MS);
}

function wireFlip(): void {
  window.addEventListener("pointerdown", () => (dragMoved = false), true);
  app.addEventListener("click", (e) => {
    const b = (e.target as Element).closest<HTMLElement>("[data-flip]");
    if (!b || b.closest("#picker-dlg") || dragMoved) return;
    flip(b);
  });
}

type Device = "terminal" | "scanner";
/** Item・Wallet のものをどこに「読ませた」か。ドラッグ＆ドロップとモバイルの長押しピッカーの共通の行き先 */
type Dest = { register: number; side: Side; device?: Device; cashBox?: boolean };

/** レジ画面に落とす＝スキャナに読ませる。決済端末は電子マネー・クレジットカードだけ、スキャナはバーコード（商品・決済用）とポイントカードだけを読む */
function deliver({ register, side, device, cashBox }: Dest, code?: string, wallet?: string): void {
  const toScanner = !!code || wallet === "pay:qr" || wallet === "pay:points";
  if (device === "terminal" && toScanner) return toast("toast-store", "商品・決済用バーコード・ポイントカードは、スキャナにかざしてください", false);
  if (device === "scanner" && !toScanner && !wallet?.startsWith("cash")) return toast("toast-store", "電子マネー・クレジットカードは、決済端末にかざしてください", false);
  if (code) return void send({ type: "scan", register, side, code });
  const [kind, value] = wallet!.split(":");
  if (kind === "cash") {
    if (side === "customer") return toast("toast-store", "現金はスタッフ側の入金ボックスに入れてください（お客様側はキャッシュレス専用です）", false);
    send(cashDrop(register, Number(value)));
  } else {
    if (cashBox) return toast("toast-store", "キャッシュレスは、お客様側の決済端末かスキャナにかざしてください", false);
    send({ type: "tap_emoney", register, method: value as PaymentMethod });
  }
}

const MOBILE = window.matchMedia("(max-width: 760px)");
const isMobile = (): boolean => MOBILE.matches;
const LONG_PRESS_MS = 500;
let longPress: { timer: number; x: number; y: number; id: number } | null = null;
let swallowClick = false;
let pickFor: Dest | null = null;

const pickerDlg = (): HTMLDialogElement => document.getElementById("picker-dlg") as HTMLDialogElement;

/** モバイルの長押しピッカー。デスクトップの「レジ画面＝スキャナ」をそのまま写す：
 *  レジ画面は Item を上・Wallet を下、スキャナは Wallet を上・Item を下に並べ、どちらからでも全部を読ませられる。
 *  入金ボックス（現金）と決済端末（かざす）は Wallet だけ */
function openPicker(dest: Dest): void {
  pickFor = dest;
  const { device, cashBox } = dest;
  const area = (label: string, html: string) => `<section class="picker-area"><h3 class="picker-area-head">${label}</h3>${html}</section>`;
  const items = area("Item", shelfPanel(snap));
  const wallet = area("Wallet", walletPanel(snap.emoney_balance));
  const body = document.getElementById("picker-body")!;
  body.innerHTML = cashBox || device === "terminal" ? wallet : device === "scanner" ? wallet + items : items + wallet;
  applyFlips(body);
  pickerDlg().showModal();
}

function pick(code?: string, wallet?: string): void {
  if (!pickFor) return;
  const dest = pickFor;
  pickerDlg().close();
  pickFor = null;
  deliver(dest, code, wallet);
}

/** Wallet のお金を入金ボックスに入れる。ふだんは金額の入力と同じ（大きい金種から崩したものとみなす）。
 *  二千円札だけは札そのものを入れ、取り扱わないので自動釣銭機がそのまま出金ボックスに吐き出す */
function cashDrop(register: number, d: number): Command {
  return d === 2000 ? { type: "insert_cash", register, denomination: d, count: 1 } : { type: "insert", register, amount: d };
}

function swipeTo(reg: number, side: Side): void {
  const el = app.querySelector<HTMLElement>(`[data-swipe="${reg}"]`);
  el?.scrollTo({ left: side === "customer" ? el.scrollWidth : 0, behavior: "smooth" });
}

function syncSwipeTabs(reg: number): void {
  const el = app.querySelector<HTMLElement>(`[data-swipe="${reg}"]`);
  const side: Side = el && el.scrollLeft > el.clientWidth / 2 ? "customer" : "staff";
  app.querySelectorAll<HTMLElement>(`[data-swipe-to^="${reg}:"]`).forEach((b) => {
    const on = b.dataset.swipeTo === `${reg}:${side}`;
    b.classList.toggle("on", on);
    b.setAttribute("aria-selected", String(on));
  });
  const io = app.querySelector(`[data-swipe="${reg}"]`)?.parentElement?.querySelector(".money-io");
  io?.classList.toggle("face-staff", side === "staff");
  io?.classList.toggle("face-customer", side === "customer");
}

function cancelLongPress(): void {
  if (longPress) clearTimeout(longPress.timer);
  longPress = null;
}

const MENU_SWIPE_PX = 40;
function wireForceRegister(): void {
  let hold: { timer: number; x: number; y: number; id: number } | null = null;
  const cancel = () => {
    if (hold) clearTimeout(hold.timer);
    hold = null;
  };
  app.addEventListener("pointerdown", (e) => {
    const b = (e.target as Element).closest<HTMLElement>("[data-ffforce]");
    if (!b || e.button !== 0) return;
    cancel();
    const cmd = b.dataset.ffforce!;
    const timer = window.setTimeout(() => {
      hold = null;
      send(JSON.parse(cmd) as Command);
    }, LONG_PRESS_MS);
    hold = { timer, x: e.clientX, y: e.clientY, id: e.pointerId };
  });
  window.addEventListener("pointermove", (e) => {
    if (hold && e.pointerId === hold.id && Math.hypot(e.clientX - hold.x, e.clientY - hold.y) > 10) cancel();
  });
  for (const ev of ["pointerup", "pointercancel"] as const) window.addEventListener(ev, cancel);
  window.addEventListener("blur", cancel);
  app.addEventListener("contextmenu", (e) => {
    if ((e.target as Element).closest("[data-ffforce]")) e.preventDefault();
  });
}

function wireMenuSwipe(): void {
  let start: { reg: number; x: number; y: number; id: number } | null = null;
  app.addEventListener("pointerdown", (e) => {
    const head = (e.target as Element).closest<HTMLElement>(".staff-screen .screen-head");
    if (!head || e.button !== 0 || (e.target as Element).closest("[data-hold]")) return;
    const reg = Number(head.dataset.region!.split("-")[1]);
    start = { reg, x: e.clientX, y: e.clientY, id: e.pointerId };
  });
  const end = (e: PointerEvent) => {
    if (!start || e.pointerId !== start.id) return;
    const { reg, x, y } = start;
    start = null;
    const dx = e.clientX - x;
    const dy = e.clientY - y;
    if (Math.abs(dx) < MENU_SWIPE_PX || Math.abs(dy) > Math.abs(dx) / 2) return;
    const r = snap.registers.find((v) => v.id === reg);
    if (!r || !storeMenuUsable(r)) return;
    if (dx > 0 && !r.menu) send({ type: "open_menu", register: reg, screen: "top" });
    if (dx < 0 && r.menu) send({ type: "open_menu", register: reg, screen: null });
  };
  window.addEventListener("pointerup", end);
  window.addEventListener("pointercancel", () => (start = null));
}

function wireSwipe(): void {
  app.addEventListener("click", (e) => {
    const b = (e.target as Element).closest<HTMLElement>("[data-swipe-to]");
    if (!b) return;
    const [reg, side] = b.dataset.swipeTo!.split(":");
    swipeTo(Number(reg), side as Side);
  });
  app.querySelectorAll<HTMLElement>("[data-swipe]").forEach((el) => {
    el.addEventListener("scroll", () => syncSwipeTabs(Number(el.dataset.swipe)), { passive: true });
  });
  app.addEventListener("pointerdown", (e) => {
    if (!isMobile() || e.button !== 0 || pickerDlg().open) return;
    const t = e.target as Element;
    if (t.closest("[data-ffforce]")) return;
    const inlet = t.closest<HTMLElement>("[data-inlet], [data-terminal], [data-scanner]");
    const screen = t.closest(".screen-head, .screen-items") ? null : t.closest<HTMLElement>("[data-drop]");
    if (!inlet && !screen) return;
    cancelLongPress();
    const timer = window.setTimeout(() => {
      longPress = null;
      swallowClick = true;
      if (inlet) {
        const register = Number(inlet.dataset.inlet ?? inlet.dataset.terminal ?? inlet.dataset.scanner);
        openPicker(
          inlet.dataset.inlet
            ? { register, side: "staff", cashBox: true }
            : { register, side: "customer", device: inlet.dataset.terminal ? "terminal" : "scanner" },
        );
      } else {
        const [reg, side] = screen!.dataset.drop!.split(":");
        openPicker({ register: Number(reg), side: side as Side });
      }
    }, LONG_PRESS_MS);
    longPress = { timer, x: e.clientX, y: e.clientY, id: e.pointerId };
  });
  window.addEventListener("pointermove", (e) => {
    if (longPress && e.pointerId === longPress.id && Math.hypot(e.clientX - longPress.x, e.clientY - longPress.y) > 10) cancelLongPress();
  });
  for (const ev of ["pointerup", "pointercancel"] as const) window.addEventListener(ev, cancelLongPress);
  window.addEventListener(
    "click",
    (e) => {
      if (!swallowClick) return;
      swallowClick = false;
      e.stopPropagation();
      e.preventDefault();
    },
    true,
  );
  window.addEventListener("pointerdown", () => (swallowClick = false), true);
  app.addEventListener("contextmenu", (e) => {
    if (isMobile() && (e.target as Element).closest("[data-drop], [data-inlet], [data-terminal], [data-scanner]")) e.preventDefault();
  });
  // ピッカーの商品・決済ボタンは長押しで読ませる。タップは空けておき、裏のある商品ボタンを裏返す
  const dlg = pickerDlg();
  let pickHold: { timer: number; x: number; y: number; id: number } | null = null;
  const cancelPickHold = () => {
    if (pickHold) clearTimeout(pickHold.timer);
    pickHold = null;
  };
  dlg.addEventListener("pointerdown", (e) => {
    const b = (e.target as Element).closest<HTMLElement>("[data-code], [data-wallet]");
    if (!b || e.button !== 0) return;
    cancelPickHold();
    const timer = window.setTimeout(() => {
      pickHold = null;
      swallowClick = true;
      pick(b.dataset.code, b.dataset.wallet);
    }, LONG_PRESS_MS);
    pickHold = { timer, x: e.clientX, y: e.clientY, id: e.pointerId };
  });
  dlg.addEventListener("pointermove", (e) => {
    if (pickHold && e.pointerId === pickHold.id && Math.hypot(e.clientX - pickHold.x, e.clientY - pickHold.y) > 10) cancelPickHold();
  });
  for (const ev of ["pointerup", "pointercancel"] as const) dlg.addEventListener(ev, cancelPickHold);
  dlg.addEventListener("close", cancelPickHold);
  dlg.addEventListener("contextmenu", (e) => {
    if ((e.target as Element).closest("[data-code], [data-wallet]")) e.preventDefault();
  });
  dlg.addEventListener("click", (e) => {
    const t = e.target as Element;
    if (t.closest("[data-close-picker]") || t === dlg) {
      dlg.close();
      pickFor = null;
      return;
    }
    const b = t.closest<HTMLElement>("[data-flip]");
    if (b) flip(b);
  });
}

boot().catch((e) => {
  app.innerHTML = `<p class="error">読み込みに失敗しました：${esc(e)}</p>`;
});
