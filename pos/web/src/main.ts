import { applyLaw, fitForms, fitText, MOBILE as MOBILE_MQ } from "./law";
import init, { PosSimulator } from "./wasm/generated/janus_pos.js";
import type { CashOutKind, Command, FfTab, FfTabDisplay, PaymentMethod, RegisterView, Rounding, Side, Snapshot, TaxKind } from "./types";
import type { CancelStage, GuideKind, RulesTab } from "./view";
import { beepError, beepPress, beepScan, chimeOk, chimePay, setBellRinging, unlockAudio } from "./sound";
import logoUrl from "./logo.jpg";
import { type Lang, langOf, localize, resetLangs, toggleLang, trText } from "./i18n";
import {
  couponCancel,
  couponCancelTarget,
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
  ownerAuthForm,
  rulesPad,
  rulesPadPopup,
  rulesValue,
  RULES_PAD,
  type RulesPadTarget,
  padPopup,
  PAD,
  padValue,
  padValueKey,
  type PadTarget,
  screenHead,
  isoDate,
  scanGuide,
  cancelAsk,
  ffFloorOpen,
  ffOpenFoot,
  parcelError,
  bellPanel,
  bellSettings,
  ownerPanel,
  shelfPanel,
  walletPanel,
  sideOf,
  sharedScreen,
  REJECT_SHOW_MS,
  rejectFlash,
  shownOnScreen,
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
  setupHold();
  window.addEventListener("pagehide", save);
  window.setInterval(frame, 100);
  // レイアウトの法則は部品の大きさで決まるので、段の大きさが変わったときとフォントが届いたときにも当て直す
  const relaid = new ResizeObserver((es) => es.forEach((e) => (fitText(e.target), fitForms(e.target), applyLaw(e.target), fitNotes(e.target))));
  app.querySelectorAll(".screen-main, .screen-items").forEach((el) => relaid.observe(el));
  void document.fonts?.ready.then(() => (fitText(app), fitForms(document), applyLaw(app), fitNotes(app)));
}

/** 最下段の添え書きが一行に入りきらないときは、文字を縮めずに電光掲示板のように流す（user 2026-11-20）。
 *  文頭で止まって読ませてから文末まで流し、文末でまた止まる。流す距離＝はみ出した幅 */
function fitNotes(root: Element): void {
  root.querySelectorAll<HTMLElement>(".call-note").forEach((p) => {
    const span = p.firstElementChild as HTMLElement | null;
    if (!span) return;
    const over = Math.ceil(span.offsetWidth - p.clientWidth);
    p.classList.toggle("marquee", over > 0);
    if (over > 0) p.style.setProperty("--marquee", `${-over}px`);
    else p.style.removeProperty("--marquee");
  });
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

/** レジの台数（シミュレーターの設定）。複数台運用は最大 5 台（user 2026-11-20）。
 *  モバイルは 2 台ちょうど（複数台運用の最低台数。3 台以上のデモは現実的でなく、1 台では複数台のデモができない、user 2026-11-20）。
 *  デスクトップは 2 台から（user 2026-11-21：1 台では、呼び出しの通知が発生したレジにだけ出ることや、
 *  チケットの取引が僅差で重なるケースのデモができない）
 *  台数を変えると店の状態を作り直す（コアは作るときに台数を決める） */
const REG_COUNT_KEY = "janus-pos-registers";
const DEFAULT_REGISTERS = 2;
const isMobileMode = (): boolean => window.matchMedia(MOBILE_MQ).matches;
const minRegisters = (): number => 2;
const maxRegisters = (): number => (isMobileMode() ? 2 : 5);

function savedRegCount(): number {
  try {
    const n = Number(localStorage.getItem(REG_COUNT_KEY));
    return Number.isInteger(n) && n >= 1 ? Math.min(Math.max(n, minRegisters()), maxRegisters()) : DEFAULT_REGISTERS;
  } catch {
    return DEFAULT_REGISTERS;
  }
}

/** デスクトップは、レジ全体（見出し・画面・入出金ボックス）が画面の高さに収まるよう、レジ画面の W の上限 --W-max を決める。
 *  画面の高さは W×√2（styles.css）。画面より上（見出し）と下（入出金ボックス）の高さは W によらないので、いまの配置から測る。
 *  モバイルは縦にスクロールするので止めない */
function fitRegisters(): void {
  const root = document.documentElement;
  if (window.matchMedia(MOBILE_MQ).matches) {
    root.style.removeProperty("--W-max");
    return;
  }
  const reg = app.querySelector<HTMLElement>(".register");
  const screen = reg?.querySelector<HTMLElement>(".screen");
  const floor = app.querySelector<HTMLElement>(".floor");
  if (!reg || !screen || !floor) return;
  const top = screen.getBoundingClientRect().top + window.scrollY;
  const below = reg.getBoundingClientRect().bottom - screen.getBoundingClientRect().bottom + (parseFloat(getComputedStyle(floor).paddingBottom) || 0);
  const W = Math.floor((window.innerHeight - top - below) / Math.SQRT2 - 2);
  root.style.setProperty("--W-max", `${Math.max(160, Math.min(512, W))}px`);
}

/** ページ上部の「使い方」 */
const HOW_TO =
  "Item（売り場の商品）と Wallet（お金・カード）は、レジの画面・入金ボックス・決済端末・スキャナを右クリック（タッチは長押し）して選びます。選んだものは、その機器で読んだことになります";

function forget(): void {
  try {
    localStorage.removeItem(SAVE_KEY);
  } catch {
    return;
  }
}

function start(): void {
  let restored = restore();
  // 最低台数に足りない保存状態（以前の 1 台）は捨てて、既定の台数で作り直す
  if (restored && (JSON.parse(restored.snapshot()) as Snapshot).registers.length < minRegisters()) {
    forget();
    restored = null;
  }
  sim = restored ?? new PosSimulator(Date.now() & 0xffff, savedRegCount(), 3);
  snap = JSON.parse(sim.snapshot()) as Snapshot;
  t0 = performance.now() - snap.now;
  buildSkeleton();
  fitRegisters();
  window.addEventListener("resize", fitRegisters);
  fitForms(app);
  app.querySelector<HTMLSelectElement>("[data-reg-count]")?.addEventListener("change", (e) => {
    const sel = e.target as HTMLSelectElement;
    if (!confirm(`レジを${sel.value}台にして、最初からやり直しますか`)) {
      sel.value = String(snap.registers.length);
      return;
    }
    try {
      localStorage.setItem(REG_COUNT_KEY, sel.value);
    } catch {
      /* 保存できなければ既定の台数で起動する */
    }
    discarding = true;
    forget();
    location.reload();
  });
  app.querySelector("[data-reset]")?.addEventListener("click", () => {
    if (!confirm("保存されている状態を消して、最初からやり直しますか")) return;
    discarding = true;
    forget();
    location.reload();
  });
  wireSwipe();
  wireMainScroll();
  wireForceRegister();
  rulesDlg().addEventListener("close", lockRules);
  // テンキーを開いているあいだの Esc はテンキーだけを閉じる（ダイアログは閉じない）
  rulesDlg().addEventListener("cancel", (e) => {
    if (!rulesPad.entry) return;
    e.preventDefault();
    rulesPadKey("cancel");
  });
  render();
}


function buildSkeleton(): void {
  const regs = snap.registers
    .map(
      (r) => `<section class="register" data-register="${r.id}" aria-label="レジ${r.id}">
      <h2>レジ${r.id}
        <label class="login-sel">スタッフ
          <select data-login="${r.id}" aria-label="レジ${r.id}のスタッフログイン">
            <option value="">未ログイン</option>
            ${snap.accounts.map((a) => `<option value="${esc(a.barcode)}">${esc(a.name)}${a.role === "owner" ? "（オーナー）" : ""}</option>`).join("")}
          </select>
        </label>
      </h2>
      <div class="tabs swipe-tabs" role="tablist" aria-label="表示する画面">
        <button type="button" role="tab" class="tab on" aria-selected="true" data-swipe-to="${r.id}:counter">レジカウンター側</button>
        <button type="button" role="tab" class="tab" aria-selected="false" data-swipe-to="${r.id}:floor">売り場側</button>
      </div>
      <div class="screens" data-swipe="${r.id}" data-shown="counter">
        <div class="screen-wrap" data-wrap="${r.id}:counter">
          <div class="screen-cap">レジカウンター側</div>
          <section class="screen counter-screen" data-screen="${r.id}:counter">
            <div class="screen-head" data-region="head-${r.id}-counter"></div>
            <div class="screen-body"><div class="screen-main" data-region="counter-${r.id}"></div><div data-region="foot-${r.id}-counter"></div></div>
            <div class="screen-items" data-region="items-${r.id}-counter"></div>
            <div data-region="pop-${r.id}-counter"></div>
          </section>
        </div>
        <div class="screen-wrap" data-wrap="${r.id}:floor">
          <div class="screen-cap">売り場側</div>
          <section class="screen floor-screen" data-screen="${r.id}:floor">
            <div class="screen-head" data-region="head-${r.id}-floor"></div>
            <div class="screen-body"><div class="screen-main" data-region="floor-${r.id}"></div><div data-region="foot-${r.id}-floor"></div></div>
            <div class="screen-items" data-region="items-${r.id}-floor"></div>
            <div data-region="pop-${r.id}-floor"></div>
          </section>
        </div>
      </div>
      <div class="money-io face-counter">
        <div class="money-io-counter" aria-label="レジ${r.id}の自動釣銭機（レジカウンター側の画面に付いています）">
          <div class="money-io-boxes">
            <button type="button" class="inlet" data-inlet="${r.id}" data-padopen="${r.id}:counter:inlet:" aria-label="レジ${r.id}の入金ボックス（押すとテンキーで金額を入れる）"><span data-region="inlet-${r.id}"></span></button>
            <button type="button" class="outlet" data-outlet="${r.id}" aria-label="レジ${r.id}の出金ボックス（金額をタッチで取り出す）"><span data-region="outlet-${r.id}"></span></button>
          </div>
        </div>
        <div class="money-io-floor" aria-label="レジ${r.id}の決済端末とスキャナ（売り場側の画面に付いています）">
          <div class="money-io-boxes">
            <div><button type="button" class="terminal" data-terminal="${r.id}" aria-label="レジ${r.id}の決済端末（電子マネー・クレジットカードをかざす）"><span class="placeholder">決済端末</span></button></div>
            <div><button type="button" class="terminal scanner" data-scanner="${r.id}" aria-label="レジ${r.id}のスキャナ（商品・決済用バーコード・ポイントカードを読む）"><span class="placeholder">スキャナ</span></button></div>
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
      <div class="top-tools">
      <span class="tip how-to" tabindex="0" data-tip="${esc(HOW_TO)}" aria-label="使い方：${esc(HOW_TO)}"><span class="how-to-label">使い方</span> <span class="info-icon" aria-hidden="true">i</span></span>
      <label class="reg-count">レジ
        <select data-reg-count aria-label="レジの台数（変えると最初からやり直します）">${Array.from({ length: Math.max(maxRegisters(), snap.registers.length) }, (_, i) => i + 1)
          .filter((n) => n >= minRegisters() || n === snap.registers.length)
          .map((n) => `<option value="${n}"${n === snap.registers.length ? " selected" : ""}>${n}台</option>`)
          .join("")}</select>
      </label>
      <button type="button" class="neg small reset-btn" data-reset title="保存されている状態を消して最初からやり直す">リセット</button>
      </div>
    </header>
    <article class="floor">
        <section class="bell-area" aria-label="Bell（呼び鈴）">
          <h2>Bell</h2>
          <div class="bells" data-region="bells"></div>
          <button type="button" class="icon-btn bell-open" data-open-bell aria-haspopup="dialog" aria-label="Bell の設定" title="Bell の設定">⚙</button>
          <div class="toast" id="toast-store"></div>
        </section>
        <div class="registers" style="--n: ${snap.registers.length}">${regs}</div>
    </article>
    <footer class="bottom">
      <p class="copyright"><a href="https://github.com/masamoto1982" target="_blank" rel="noopener">© 2026 masamoto yamashiro</a></p>
    </footer>
    <dialog class="rules-dlg" id="rules-dlg" aria-labelledby="rules-title">
      <div class="dlg-head"><h2 id="rules-title">販売規則の設定</h2><button class="small ghost" data-close-rules>閉じる</button></div>
      <div class="owner-auth" data-region="owner-auth"></div>
      <div class="toast" id="toast-rules"></div>
      <div data-region="owner"></div>
      <div data-region="rules-pad"></div>
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
    ${rampFilter()}`;
}

// グレーの四段（styles.css の --grey-1〜4）を青の四段（--blue-1〜4）に写す
const RAMP_STEPS: [number, string][] = [
  [0x00, "oklch(0 0 0)"],
  [0x53, "oklch(0.44 0.13 262.2)"],
  [0x69, "oklch(0.542 0.254 262.2)"],
  [0xd4, "oklch(0.87 0.072 262.2)"],
  [0xee, "oklch(0.95 0.028 262.2)"],
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
  fitText(el);
  fitForms(el);
  applyLaw(el);
  fitNotes(el);

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
    const rows: [number, number][] = [...r.counter.lines.map((l): [number, number] => [l.id, l.qty]), ...r.counter.coupons.map((c): [number, number] => [c.id, 1])];
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
  const v = r.counter;
  return (
    r.held ||
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
    app.querySelectorAll<HTMLElement>(`[data-screen^="${reg}:"] .screen-main`).forEach((el) => {
      el.scrollTop = 0;
      mainPage.delete(pageKey(el));
      const box = el.querySelector<HTMLElement>(":scope > .lines-scroll");
      if (box) box.scrollTop = 0;
    });
  }
  movedRegs.clear();
  for (const r of snap.registers) {
    const busy = inTransaction(r);
    if (busyRegs.has(r.id) && !busy) resetLangs(r.id);
    if (busy) busyRegs.add(r.id);
    else busyRegs.delete(r.id);
    const sel = app.querySelector<HTMLSelectElement>(`[data-login="${r.id}"]`);
    if (sel && document.activeElement !== sel) {
      const acc = r.counter.state === "absent" ? undefined : snap.accounts.find((a) => a.name === r.counter.name);
      sel.value = acc?.barcode ?? "";
    }
    for (const side of ["counter", "floor"] as const) {
      const head = app.querySelector<HTMLElement>(`[data-region="head-${r.id}-${side}"]`);
      if (!head) continue;
      head.classList.toggle("sos", r.emergency);
      head.classList.toggle("calling", r.lamp && !r.emergency);
      const lang = langOf(r.id, side);
      app.querySelector(`[data-screen="${r.id}:${side}"]`)?.setAttribute("lang", lang);
      if (r.lamp) {
        head.title = lang === "en" ? trText("呼び出し中") : "呼び出し中";
      } else {
        head.removeAttribute("title");
      }
    }
    // 第一待ち受け：取引のない待ち受けで、レジ前に人がいない（カーソルが重なっていない）状態が FIRST_WAIT_AFTER_MS 続いたとき
    let counterMain = sharedScreen(r, snap, "counter");
    let floorMain = sharedScreen(r, snap, "floor");
    const vacant = isWelcome(counterMain) && isWelcome(floorMain) && !busy && !r.counter.point_card && !r.floor.point_card;
    if (!vacant || present(r.id)) lastBusy.set(r.id, Date.now());
    const first = vacant && Date.now() - (lastBusy.get(r.id) ?? 0) >= FIRST_WAIT_AFTER_MS;
    if (first) {
      counterMain = firstWelcome(r, "counter");
      floorMain = firstWelcome(r, "floor");
    }
    app.querySelector(`[data-swipe="${r.id}"]`)?.classList.toggle("first-wait", first);
    for (const side of ["counter", "floor"] as const) {
      const entry = qtyEntry.get(`${r.id}:${side}`);
      if (entry && (r.counter.phase !== "registering" || !r.counter.lines.some((l) => l.id === entry.id))) qtyEntry.delete(`${r.id}:${side}`);
      // 「FF」から開いた FF タブは、セルフ操作モードを抜けたとき・呼び出しが立ったとき・取引が終わったときに閉じる（四つのボタンに戻る）
      if (r.mode !== "self_operated" || r.waiting_calls.length || r.last_receipt) ffFloorOpen.delete(r.id);
      const items = itemsPanel(r, snap, ffUi(r.id), side, Date.now(), first);
      const locked = r.active_side !== null && r.active_side !== side;
      const el = app.querySelector(`[data-screen="${r.id}:${side}"]`);
      el?.classList.toggle("inactive", locked);
      const operable = r.active_side !== null ? r.active_side === side : r.mode !== "staff_operated" || side === "counter";
      el?.classList.toggle("operable", operable);
      el?.classList.toggle("mode-staff", r.mode === "staff_operated");
      el?.classList.toggle("mode-self", r.mode === "self_operated");
      el?.classList.toggle("calling", side === "counter" && r.lamp);
      el?.classList.toggle("head-blink", r.lamp || r.emergency);
      el?.classList.toggle("lamp", r.lamp && !r.emergency);
      el?.classList.toggle("sos", r.emergency);
      app.querySelector(`[data-swipe-to="${r.id}:${side}"]`)?.classList.toggle("operable", operable);
      const wrap = el?.parentElement;
      wrap?.classList.toggle("inactive", locked);
      const lang = langOf(r.id, side);
      setRegion(`head-${r.id}-${side}`, screenHead(r, new Date(), side), lang);
      setRegion(`items-${r.id}-${side}`, items, lang);
      setRegion(`foot-${r.id}-${side}`, items.includes("ff-open") ? ffOpenFoot(r) : "", lang);
      const pad = padEntry.get(`${r.id}:${side}`);
      if (pad && pad.target !== "inlet" && !el?.querySelector(`[data-padopen="${r.id}:${side}:${pad.target}:${pad.line ?? ""}"]`)) padEntry.delete(`${r.id}:${side}`);
      if (side === "counter" && !["refund", "emergency", "expense"].includes(r.menu?.screen ?? "")) {
        padValue.delete(padValueKey(r.id, "mamt"));
        padValue.delete(padValueKey(r.id, "mpw"));
      }
      // 荷物の長さ・重さ・郵便番号は受付中の荷物のものだけ残す。明細の番号は取引ごとに振り直すので、
      // 残しておくと次の取引の荷物に前の荷物の値が入ってしまう
      if (side === "counter") {
        const pending = new Set(r.counter.lines.filter((l) => l.parcel && l.kind === "service" && !l.done).map((l) => String(l.id)));
        for (const key of [...padValue.keys()]) {
          const [reg, target, line] = key.split(":");
          if (reg === String(r.id) && ["pcm", "pkg", "post"].includes(target) && !pending.has(line)) padValue.delete(key);
        }
      }
      // ポイントの使い方の画面を離れたら、指定しかけたポイント数を捨てる
      if (r.floor.method !== "points" || r.prepaid) padValue.delete(padValueKey(r.id, "ppt"));
      setRegion(`pop-${r.id}-${side}`, qtyPopup(r, side) || padPopup(r, side), lang);
      el?.classList.toggle("no-items", !items);
    }
    const face: Side = r.menu ? "counter" : r.mode === "staff_operated" ? "counter" : "floor";
    if (face !== activeShown.get(r.id)) {
      activeShown.set(r.id, face);
      swipeTo(r.id, face);
    }
    setRegion(`counter-${r.id}`, counterMain, langOf(r.id, "counter"));
    setRegion(`floor-${r.id}`, floorMain, langOf(r.id, "floor"));
    slideOnHold(r);
    setRegion(
      `outlet-${r.id}`,
      r.outlet ? `<b title="${r.outlet_detail.map(([d, n]) => `${d.toLocaleString()}円×${n}`).join(" ")}">${r.outlet.toLocaleString()}円</b>` : `<span class="placeholder">出金</span>`,
    );
    app.querySelector(`[data-outlet="${r.id}"]`)?.classList.toggle("blink", r.outlet > 0);
    const inlet = app.querySelector<HTMLElement>(`[data-inlet="${r.id}"]`);
    const terminal = app.querySelector<HTMLElement>(`[data-terminal="${r.id}"]`);
    const paying = !r.menu && r.counter.phase === "paying" && r.counter.method !== null && r.emoney_short === null && r.emoney_partial === null;
    const emoney = r.menu ? null : r.emoney;
    const partial = !r.menu && r.emoney_partial !== null && r.emoney_partial_ok;
    const cashPay = paying && r.counter.method === "cash";
    const charging = !!emoney && emoney.kind === "charge";
    const tapping = !!emoney && emoney.kind !== "menu" && !(charging && !emoney.inserted);
    if (inlet) {
      inlet.classList.toggle("blink", cashPay || (charging && !emoney!.inserted));
      const hint = cashPay || (charging && !emoney!.inserted) ? (charging ? "紙幣を入れる" : "現金を入れる") : "入金";
      const shown = emoney ? emoney.inserted : !r.menu ? r.counter.inserted : 0;
      setRegion(`inlet-${r.id}`, shown ? `<b>${shown.toLocaleString()}円</b>` : `<span class="placeholder">${esc(hint)}</span>`);
    }
    if (terminal) {
      // 決済端末は、電子マネー・クレジットカードを待っているあいだだけ青白くゆったり明滅する（スキャナは待機中は光らない）。
      // かざした直後に決済が終わっても、光が広がり終わるまでは待ち受けの見た目のまま
      const reader = (paying && READER_METHODS.has(r.counter.method!)) || partial || tapping || terminal.classList.contains("fx-tap");
      terminal.classList.toggle("reader", reader);
    }
  }
  setRegion("bells", bellPanel(snap));
  setRegion("bell-settings", bellSettings(snap));
  setRegion("owner-auth", ownerAuth ? "" : ownerAuthForm());
  setRegion("owner", ownerAuth ? ownerPanel(snap, rulesTab) : "");
  setRegion("rules-pad", rulesPadPopup(snap));
  app.querySelectorAll<HTMLElement>(".screen-main").forEach((m) => paginate(m));
}


const ffTabs = new Map<number, FfUi>();
const ffUi = (reg: number): FfUi => ffTabs.get(reg) ?? { tab: FF_TABS[0][0], page: 0 };

function frame(): void {
  sim.tick(performance.now() - t0);
  snap = JSON.parse(sim.snapshot()) as Snapshot;
  setBellRinging(
    snap.bells.some((m) => m.buzzing),
    Math.max(0, ...snap.registers.map((r) => r.ring_volume)),
    snap.registers.some((r) => r.emergency),
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
  el.className = `toast show ${error ? "toast-error" : ""}`;
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
    const sum = snap.registers.find((r) => r.id === register)?.counter.lines.find((l) => l.id === line)?.parcel?.sum_cm;
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
      send({ type: "hand_tickets", register, side: "counter", handed: value });
    } else if (e.target === "tkr") {
      if (e.digits === "") return;
      send({ type: "receive_tickets", register, side: "floor", received: value });
    } else if (e.target === "ffv") {
      if (e.digits === "") return;
      const ffKey = `${register}:${side}:${e.line}`;
      ffvInput.set(ffKey, String(value));
      send({ type: "check_ff", register, side: side as Side, line_id: e.line!, handed: value });
    } else if (e.target === "cpn") {
      // 取り消す枚数：新しく読んだクーポンから、その枚数だけ取り消す（返却の案内が出る）
      if (e.digits === "") return;
      const r = snap.registers.find((x) => x.id === register);
      const coupons = (side === "counter" ? r?.counter.coupons : r?.floor.coupons) ?? [];
      const cs = couponCancelTarget(register, side as Side, coupons) ?? [];
      if (value < 1 || value > cs.length) {
        e.error = `1〜${cs.length}枚で入力してください`;
        frame();
        return;
      }
      for (const c of cs.slice(-value).reverse()) send({ type: "remove_coupon", register, side: side as Side, coupon_id: c.id });
      couponCancel.delete(`${register}:${side}`);
      padValue.delete(padValueKey(register, "cpn"));
    } else if (e.target in SUBMIT) {
      // 入力欄ひとつの画面：テンキーの「決定」でそのまま送る。受け付けられなければ理由を出してテンキーを開いたままにする
      if (e.digits === "") return;
      const cmd = SUBMIT[e.target]!(register, side as Side, e.digits, e.line);
      if (!cmd || !send(cmd, true)) {
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
    const lines = side === "counter" ? r?.counter.lines ?? [] : r?.floor.lines ?? [];
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
/** 理由をほかの場所で出すコマンド（荷物の入力欄）は、登録画面に重ねない */
const OWN_ERROR = new Set(["parcel_length", "parcel_size", "parcel_postal", "parcel_back", "parcel_next"]);

/** レジが受け付けなかった理由を、操作した側の登録画面の中央に一瞬だけ重ねる（user 2026-11-18〜19）。
 *  登録画面の最上部に赤褐色の一行を出す形（旧 B-2）は全廃した。背後の登録画面はぼかして読めなくする。
 *  案内は画面の描き直しの外に置くので、表示中に画面が変わっても最後まで出る。ポインターは素通しにし、表示中もすべての操作を受け付ける */
function flashRejection(reg: number, side: Side, reason: string, done = false): void {
  const body = app.querySelector(`[data-region="${side}-${reg}"]`)?.parentElement;
  if (!body) return;
  body.querySelector(".reject-flash")?.remove();
  const el = document.createElement("div");
  el.className = done ? "reject-flash done" : "reject-flash";
  el.setAttribute("role", "alert");
  el.style.setProperty("--flash-ms", `${REJECT_SHOW_MS}ms`);
  el.innerHTML = rejectFlash(langOf(reg, side) === "ja" ? reason : trText(reason));
  body.append(el);
  el.addEventListener("animationend", () => el.remove());
  window.setTimeout(() => el.remove(), REJECT_SHOW_MS + 200);
}

/** quiet：理由をテンキーの中など呼び出し元で出すときは、登録画面に重ねない */
function send(cmd: Command, quiet = false): boolean {
  const fx = deviceFx(cmd);
  // 「商品を取り消す」を選んだあとに読んだ商品は登録せず、その商品を1点取り消す（一度きり。スタッフバーコードはふだんどおり）
  const asked = cmd.type === "scan" && !cmd.code.startsWith(STAFF_BARCODE_PREFIX) ? cancelAsk.get(cmd.register) : undefined;
  if (cmd.type === "scan" && asked?.stage === "item") {
    cancelAsk.delete(cmd.register);
    cmd = { type: "cancel_item", register: cmd.register, side: asked.side, code: cmd.code };
  }
  // 最下段の FF ボタンで取り消したときも一度きり（user 2026-11-21）
  if (cmd.type === "cancel_item") cancelAsk.delete(cmd.register);
  sim.tick(performance.now() - t0);
  const res = JSON.parse(sim.dispatch(JSON.stringify(cmd))) as { ok: boolean; message?: string; error?: string };
  lastError = res.ok ? undefined : res.error;
  // 取引を取り消して保留した取引に戻ったときも、取消しの画面は閉じる（戻った取引に持ち越さない）
  if (res.ok && cmd.type === "void_transaction") cancelAsk.delete(cmd.register);
  // 取り消せたら、登録済みだったことを操作した側の登録画面の中央に一瞬だけ知らせる
  if (res.ok && cmd.type === "cancel_item" && res.message) flashRejection(cmd.register, cmd.side, res.message, true);
  const where = RULE_CMDS.includes(cmd.type) ? "toast-rules" : sideOf(cmd) === "store" ? "toast-store" : null;
  if (where) {
    if (res.ok) {
      if (res.message) toast(where, res.message, false);
    } else toast(where, res.error ?? "操作できません", true);
  }
  if (!res.ok && !where && !quiet && "register" in cmd) {
    const side = sideOf(cmd);
    const reason = res.error ?? "操作できません";
    if (side !== "store" && !OWN_ERROR.has(cmd.type) && !shownOnScreen(reason)) flashRejection(cmd.register, side, reason);
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
  if (["scan", "cancel_item", "register_ff", "tap_emoney"].includes(cmd.type)) (res.ok && !expired && !emoneyShort ? (paidAtTerminal ? chimePay : beepScan) : beepError)();
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
    if (!el.isConnected) return;
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
  scope = b.closest("[data-screen]") ?? app;
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
  if (sel.dataset.fftabDisplay && ownerAuth) {
    const ok = send({ type: "set_ff_tab_display", ...ownerAuth, tab: sel.dataset.fftabDisplay as FfTab, display: sel.value as FfTabDisplay });
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
  if (sel.value) send({ type: "scan", register, side: "counter", code: sel.value });
  else send({ type: "end_staff_mode", register });
  sel.blur();
});

const RULES_GEAR = `<button type="button" class="icon-btn rules-open" data-open-rules aria-haspopup="dialog" aria-label="販売規則の設定" title="販売規則の設定">⚙</button>`;
const rulesDlg = (): HTMLDialogElement => document.getElementById("rules-dlg") as HTMLDialogElement;

const RULE_CMDS = ["verify_owner", "set_product_age", "set_sale_limit", "set_ff_tab_display", "set_tax_rate", "add_tax_class", "set_product_tax", "set_invoice"];
let ownerAuth: { owner_id: string; password: string } | null = null;
let rulesTab: RulesTab = "tax";

function lockRules(): void {
  ownerAuth = null;
  rulesTab = "tax";
  rulesPad.entry = null;
  rulesValue.clear();
  render();
}

function verifyOwner(): void {
  const creds = { owner_id: rulesValue.get("oid") ?? "000", password: rulesValue.get("opw") ?? "" };
  rulesValue.delete("opw");
  if (!send({ type: "verify_owner", ...creds })) return render();
  ownerAuth = creds;
  rulesValue.delete("oid");
  render();
}

/** 販売規則の設定のテンキー。打ち方はレジ画面のテンキーと同じ（小数は税率だけ、0.01% 単位） */
function rulesPadKey(k: string): void {
  const e = rulesPad.entry;
  if (!e) return;
  const d = RULES_PAD[e.target];
  if (k !== "ok") e.error = undefined;
  if (k === "cancel") rulesPad.entry = null;
  else if (k === "back") e.digits = e.digits.slice(0, -1);
  else if (k === ".") {
    if (d.decimals && !e.digits.includes(".") && e.digits.length < d.max) e.digits = (e.digits || "0") + ".";
  } else if (/^[0-9]$/.test(k) && e.digits.replace(".", "").length < d.max) {
    const next = e.digits + k;
    const dec = next.split(".")[1];
    if (dec !== undefined && dec.length > (d.decimals ?? 0)) return;
    e.digits = d.code || next.includes(".") ? next : next.replace(/^0+(?=\d)/, "");
  } else if (k === "ok") {
    if (e.digits.endsWith(".") || (e.digits === "" && e.target !== "inv")) return;
    if (e.target === "rate") {
      // 区分の税率：欄ひとつなので、テンキーの決定でそのまま変える。受け付けられなければ理由を出してテンキーを開いたままにする
      if (!ownerAuth || !send({ type: "set_tax_rate", ...ownerAuth, class_id: e.id!, rate_bp: Math.round(Number(e.digits) * 100) }, true)) {
        e.error = lastError ?? "受け付けられません";
        return render();
      }
    } else rulesValue.set(e.target, e.digits);
    rulesPad.entry = null;
  }
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
  if (rulesPad.entry && rulesDlg().open && !e.isComposing && !(e.target as HTMLElement).closest?.("input, select, textarea")) {
    const k = /^[0-9.]$/.test(e.key) ? e.key : e.key === "Backspace" ? "back" : e.key === "Enter" ? "ok" : null;
    if (!k) return;
    e.preventDefault();
    return rulesPadKey(k);
  }
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
  if ((e.target as HTMLElement).dataset?.rpadBackdrop) return rulesPadKey("cancel");
  const padBackdrop = (e.target as HTMLElement).dataset?.padBackdrop;
  if (padBackdrop) return padKey(padBackdrop, "cancel");
  const b = (e.target as Element).closest("button");
  if (!b || b.disabled) return;
  if (b.hasAttribute("data-open-rules")) {
    // モバイルは長押しピッカーの Item の見出しから開く：ピッカーを閉じてから開く
    if (pickerDlg().open) {
      pickerDlg().close();
      pickFor = null;
    }
    rulesDlg().showModal();
    return fitForms(rulesDlg());
  }
  if (b.hasAttribute("data-owner-submit")) return verifyOwner();
  if (b.dataset.rpadopen !== undefined) {
    const [target, id] = b.dataset.rpadopen.split(":") as [RulesPadTarget, string];
    const same = rulesPad.entry?.target === target && (rulesPad.entry.id ?? "") === id;
    rulesPad.entry = same ? null : { target, id: id || undefined, digits: "" };
    return render();
  }
  if (b.dataset.rpad) return rulesPadKey(b.dataset.rpad);
  if (b.hasAttribute("data-close-rules")) return rulesDlg().close();
  if (b.dataset.rulesTab) {
    rulesTab = b.dataset.rulesTab as RulesTab;
    return render();
  }
  if (b.hasAttribute("data-add-tax") && ownerAuth) {
    const field = (k: string) => rulesDlg().querySelector<HTMLInputElement>(`[data-new-tax="${k}"]`)!;
    const rate = Number(rulesValue.get("newrate") || 0);
    if (
      send({ type: "add_tax_class", ...ownerAuth, name: field("name").value, kind: field("kind").value as TaxKind, rate_bp: Math.round(rate * 100) })
    ) {
      field("name").value = "";
      rulesValue.delete("newrate");
      render();
    }
    return;
  }
  if (b.hasAttribute("data-save-invoice") && ownerAuth) {
    const rounding = rulesDlg().querySelector<HTMLSelectElement>(`[data-invoice="rounding"]`)!.value as Rounding;
    const digits = rulesValue.get("inv") ?? snap.tax.invoice_no.replace(/^T/, "");
    if (send({ type: "set_invoice", ...ownerAuth, invoice_no: digits ? `T${digits}` : "", rounding })) rulesValue.delete("inv");
    render();
    return;
  }
  if (b.hasAttribute("data-open-bell")) return (document.getElementById("bell-dlg") as HTMLDialogElement).showModal();
  if (b.hasAttribute("data-close-bell")) return (document.getElementById("bell-dlg") as HTMLDialogElement).close();
  if (b.dataset.ffopen) {
    const reg = Number(b.dataset.ffopen);
    const first = FF_TABS.find(([k]) => snap.ff_tabs.some((t) => t.tab === k && t.display === "both"));
    ffFloorOpen.add(reg);
    if (first) ffTabs.set(reg, { tab: first[0], page: 0 });
    return frame();
  }
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
  if (b.dataset.cancelAsk) {
    const [reg, side, stage] = b.dataset.cancelAsk.split(":");
    if (stage) cancelAsk.set(Number(reg), { side: side as Side, stage: stage as CancelStage });
    else cancelAsk.delete(Number(reg));
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
  if (b.dataset.couponcancel) {
    const [reg, side, line, kind] = b.dataset.couponcancel.split(":");
    couponCancel.set(`${reg}:${side}`, { line: Number(line), kind });
    frame();
    return;
  }
  if (b.dataset.couponclose) {
    couponCancel.delete(b.dataset.couponclose);
    padValue.delete(padValueKey(Number(b.dataset.couponclose.split(":")[0]), "cpn"));
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

/** 呼び出しボタン（押しているあいだ鳴る） */
function setupHold(): void {
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

type Device = "terminal" | "scanner";
/** Item・Wallet のものをどこに「読ませた」か（ピッカーを開いた機器） */
type Dest = { register: number; side: Side; device?: Device; cashBox?: boolean };

/** レジ画面に落とす＝スキャナに読ませる。決済端末は電子マネー・クレジットカードだけ、スキャナはバーコード（商品・決済用）とポイントカードだけを読む */
function deliver({ register, side, device, cashBox }: Dest, code?: string, wallet?: string): void {
  const toScanner = !!code || wallet === "pay:qr" || wallet === "pay:points";
  if (device === "terminal" && toScanner) return toast("toast-store", "商品・決済用バーコード・ポイントカードは、スキャナにかざしてください", false);
  if (device === "scanner" && !toScanner && !wallet?.startsWith("cash")) return toast("toast-store", "電子マネー・クレジットカードは、決済端末にかざしてください", false);
  if (code) return void send({ type: "scan", register, side, code });
  const [kind, value] = wallet!.split(":");
  if (kind === "cash") {
    if (side === "floor") return toast("toast-store", "現金はレジカウンター側の入金ボックスに入れてください（売り場側はキャッシュレス専用です）", false);
    send(cashDrop(register, Number(value)));
  } else {
    if (cashBox) return toast("toast-store", "キャッシュレスは、売り場側の決済端末かスキャナにかざしてください", false);
    send({ type: "tap_emoney", register, method: value as PaymentMethod });
  }
}

const LONG_PRESS_MS = 500;
let longPress: { timer: number; x: number; y: number; id: number } | null = null;
let swallowClick = false;
let pickFor: Dest | null = null;
/** 直前に押したポインターの種類。タッチの長押しでもブラウザが contextmenu を出すので、右クリックと見分ける */
let lastPointer = "mouse";

const pickerDlg = (): HTMLDialogElement => document.getElementById("picker-dlg") as HTMLDialogElement;

/// 仕様 15.1：Item・Wallet は常には出さず、機器を右クリック（タッチは長押し）して開くピッカーから選ぶ（user 2026-11-20、デスクトップもモバイルと同じ）。
/** 「レジ画面＝スキャナ」：レジ画面は Item を上・Wallet を下、スキャナは Wallet を上・Item を下に並べ、どちらからでも全部を読ませられる。
 *  入金ボックス（現金）と決済端末（かざす）は Wallet だけ */
function openPicker(dest: Dest): void {
  pickFor = dest;
  const { device, cashBox } = dest;
  const area = (label: string, html: string) => `<section class="picker-area"><h3 class="picker-area-head">${label}</h3>${html}</section>`;
  // Item・Wallet の欄はないので、販売規則の設定の歯車はピッカーの Item の見出しの横に置く（user 2026-11-20）
  const items = area(`Item ${RULES_GEAR}`, shelfPanel(snap));
  const wallet = area("Wallet", walletPanel(snap.emoney_balance));
  document.getElementById("picker-body")!.innerHTML = cashBox || device === "terminal" ? wallet : device === "scanner" ? wallet + items : items + wallet;
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

/// 仕様 15.9：モバイルのレジカウンター側・売り場側は横スワイプかタブで切り替える（保留をボタンにしたので横スワイプを戻した、user 2026-11-20）
function swipeTo(reg: number, side: Side): void {
  const el = app.querySelector<HTMLElement>(`[data-swipe="${reg}"]`);
  if (!el) return;
  markShown(reg, side);
  if (el.scrollWidth > el.clientWidth) el.scrollTo({ left: side === "floor" ? el.scrollWidth : 0, behavior: "smooth" });
}

/** 表にしている面をタブと入出金・決済端末の出し分けに写す */
function markShown(reg: number, side: Side): void {
  const el = app.querySelector<HTMLElement>(`[data-swipe="${reg}"]`);
  if (!el) return;
  el.dataset.shown = side;
  app.querySelectorAll<HTMLElement>(`[data-swipe-to^="${reg}:"]`).forEach((b) => {
    const on = b.dataset.swipeTo === `${reg}:${side}`;
    b.classList.toggle("on", on);
    b.setAttribute("aria-selected", String(on));
  });
  const io = el.parentElement?.querySelector(".money-io");
  io?.classList.toggle("face-counter", side === "counter");
  io?.classList.toggle("face-floor", side === "floor");
}

/** ピッカーを開く機器：レジ画面（その側のスキャナ）・入金ボックス・決済端末・スキャナ */
function pickerDest(t: Element): Dest | null {
  const dev = t.closest<HTMLElement>("[data-inlet], [data-terminal], [data-scanner]");
  if (dev) {
    const register = Number(dev.dataset.inlet ?? dev.dataset.terminal ?? dev.dataset.scanner);
    return dev.dataset.inlet
      ? { register, side: "counter", cashBox: true }
      : { register, side: "floor", device: dev.dataset.terminal ? "terminal" : "scanner" };
  }
  const screen = t.closest<HTMLElement>("[data-screen]");
  if (!screen) return null;
  const [reg, side] = screen.dataset.screen!.split(":");
  return { register: Number(reg), side: side as Side };
}

function cancelLongPress(): void {
  if (longPress) clearTimeout(longPress.timer);
  longPress = null;
}

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

/** 明細のページ（user 2026-11-20）：明細はボタンでページを送る（スクロールバー・スワイプでは送らない。
 *  明細がスクロールすると、モバイルでページの縦スクロールと取り合いになる）。
 *  登録画面（明細の表 .lines.cols3）：内税の行と見出し（品目・数量・小計）は動かず、明細の行だけが .lines-scroll の中で送られる。
 *  見えている高さに入りきる行を一ページとし、ページの一行目を見出しの下に合わせる。最後のページの一行目まで上げられるよう下に余白を足す。
 *  複数ページのときは下に「&lt; Page:1/3 &gt;」（円形のボタンは FF のページ送りと同じ、端のページでは出さない）。明細の数が変わったら一ページ目に戻す。
 *  「レジ袋を登録したか」のように、ページごとに見落としなく確かめられるようにするため */
const mainPage = new Map<string, { page: number; count: number; starts: number[] }>();
const pageKey = (main: HTMLElement): string => main.dataset.region ?? "";

/** force＝描き直していなくても割り付け直す（画面の大きさが変わったとき） */
function paginate(main: HTMLElement, force = false): void {
  const table = main.querySelector<HTMLElement>(":scope > table.lines.cols3, :scope > .lines-scroll > table.lines.cols3");
  if (!table) return;
  let box = table.parentElement as HTMLElement;
  // 中身を描き直していなければ、スクロールの位置もページもそのまま
  if (box.classList.contains("lines-scroll") && !force) return;
  main.querySelector(":scope > .page-more")?.remove();
  if (!box.classList.contains("lines-scroll")) {
    box = document.createElement("div");
    box.className = "lines-scroll";
    table.replaceWith(box);
    box.append(table);
  }
  const rows = [...table.querySelectorAll<HTMLElement>("tbody > tr")].filter((r) => !r.querySelector("td.empty") && !r.classList.contains("page-pad"));
  table.querySelector("tbody > tr.page-pad")?.remove();
  rows.forEach((r) => r.classList.remove("page-start"));
  const st = mainPage.get(pageKey(main));
  const state = st && st.count === rows.length ? st : { page: 0, count: rows.length, starts: [0] };
  mainPage.set(pageKey(main), state);
  if (!rows.length) return;
  const [side, reg] = pageKey(main).split("-");
  const lang = langOf(Number(reg), side as Side);
  // ページの下の行を先に置き、その分を引いた高さで割り付ける
  const more = document.createElement("p");
  more.className = "page-more";
  more.innerHTML = `<button type="button" class="ff-arrow page-turn prev" data-linepage="-1" aria-label="${lang === "en" ? trText("前のページ") : "前のページ"}">&lt;</button><span class="page-no">Page:1/1</span><button type="button" class="ff-arrow page-turn next" data-linepage="1" aria-label="${lang === "en" ? trText("次のページ") : "次のページ"}">&gt;</button>`;
  main.append(more);
  const head = table.querySelector("thead")?.getBoundingClientRect().height ?? 0;
  const view = box.clientHeight - head;
  const heights = rows.map((r) => r.getBoundingClientRect().height);
  const starts: number[] = [0];
  if (heights.reduce((t, h) => t + h, 0) > view + 0.5) {
    let used = 0;
    for (let i = 0; i < rows.length; i++) {
      if (used > 0 && used + heights[i] > view + 0.5) {
        starts.push(i);
        used = 0;
      }
      used += heights[i];
    }
  }
  state.starts = starts;
  state.page = Math.min(state.page, starts.length - 1);
  if (starts.length === 1) {
    more.remove();
    box.scrollTop = 0;
    return;
  }
  starts.forEach((i) => rows[i].classList.add("page-start"));
  // 最後のページの一行目を一番上まで上げられるように、残りの高さを埋める
  const lastH = heights.slice(starts[starts.length - 1]).reduce((t, h) => t + h, 0);
  const pad = document.createElement("tr");
  pad.className = "page-pad";
  pad.setAttribute("aria-hidden", "true");
  pad.innerHTML = `<td colspan="3" style="height:${Math.max(0, view - lastH)}px"></td>`;
  rows[rows.length - 1].after(pad);
  showPage(main, lang);
}

const pageTop = (box: HTMLElement, rows: HTMLElement[], i: number, head: number): number =>
  rows[i].getBoundingClientRect().top - box.getBoundingClientRect().top + box.scrollTop - head;

/** 今のページの一行目を見出しの下に合わせ、「Page:2/3」とボタンを書き換える */
function showPage(main: HTMLElement, lang = langOf(Number(pageKey(main).split("-")[1]), pageKey(main).split("-")[0] as Side)): void {
  const st = mainPage.get(pageKey(main));
  const box = main.querySelector<HTMLElement>(":scope > .lines-scroll");
  const more = main.querySelector<HTMLElement>(":scope > .page-more");
  if (!st || !box || !more) return;
  const rows = [...box.querySelectorAll<HTMLElement>("tbody > tr")].filter((r) => !r.querySelector("td.empty") && !r.classList.contains("page-pad"));
  const head = box.querySelector("thead")?.getBoundingClientRect().height ?? 0;
  const page = Math.max(0, Math.min(st.page, st.starts.length - 1));
  st.page = page;
  // ほかのページの行は隠す（次のページの一行目が下に見切れて覗かないように）
  const from = st.starts[page];
  const to = st.starts[page + 1] ?? rows.length;
  rows.forEach((r, i) => r.classList.toggle("off-page", i < from || i >= to));
  if (rows[st.starts[page]]) box.scrollTop = page === 0 ? 0 : pageTop(box, rows, st.starts[page], head);
  const no = more.querySelector<HTMLElement>(".page-no")!;
  no.textContent = `Page:${page + 1}/${st.starts.length}`;
  no.setAttribute("aria-label", lang === "en" ? `Page ${page + 1} of ${st.starts.length}` : `${st.starts.length}ページ中${page + 1}ページ目`);
  // 端のページでは出さない（場所は空けたまま、Page の位置を動かさない）
  more.querySelector<HTMLElement>(".page-turn.prev")!.classList.toggle("off", page === 0);
  more.querySelector<HTMLElement>(".page-turn.next")!.classList.toggle("off", page === st.starts.length - 1);
}

function wireMainScroll(): void {
  window.addEventListener("resize", () => app.querySelectorAll<HTMLElement>(".screen-main").forEach((m) => paginate(m, true)));
  app.addEventListener("click", (e) => {
    const b = (e.target as Element).closest<HTMLElement>("[data-linepage]");
    const main = b?.closest<HTMLElement>(".screen-main");
    const st = main && mainPage.get(pageKey(main));
    if (!b || !main || !st) return;
    st.page += Number(b.dataset.linepage);
    showPage(main);
  });
}

/** 保留したら第二待ち受けを右から、保留した取引に戻ったらそれを左から滑り込ませる（両画面とも） */
const heldShown = new Map<number, boolean>();
function slideOnHold(r: RegisterView): void {
  const was = heldShown.get(r.id);
  heldShown.set(r.id, r.held);
  if (was === undefined || was === r.held) return;
  for (const side of ["counter", "floor"] as const) {
    const el = app.querySelector<HTMLElement>(`[data-region="${side}-${r.id}"]`);
    if (!el) continue;
    el.classList.remove("slide-from-right", "slide-from-left");
    void el.offsetWidth;
    el.classList.add(r.held ? "slide-from-right" : "slide-from-left");
    el.addEventListener("animationend", () => el.classList.remove("slide-from-right", "slide-from-left"), { once: true });
  }
}

function wireSwipe(): void {
  app.querySelectorAll<HTMLElement>("[data-swipe]").forEach((el) => {
    el.addEventListener(
      "scroll",
      () => {
        if (el.scrollWidth <= el.clientWidth) return;
        const side: Side = el.scrollLeft > (el.scrollWidth - el.clientWidth) / 2 ? "floor" : "counter";
        if (el.dataset.shown !== side) markShown(Number(el.dataset.swipe), side);
      },
      { passive: true },
    );
  });
  app.addEventListener("click", (e) => {
    const b = (e.target as Element).closest<HTMLElement>("[data-swipe-to]");
    if (!b) return;
    const [reg, side] = b.dataset.swipeTo!.split(":");
    swipeTo(Number(reg), side as Side);
  });
  app.addEventListener("pointerdown", (e) => {
    lastPointer = e.pointerType;
    // マウスは右クリック（下の contextmenu）。タッチ・ペンは長押し。ヘッダー（店舗メニューのスワイプ・呼び鈴）と最下段（呼び出しボタン）は長押しの対象外
    if (e.pointerType === "mouse" || e.button !== 0 || pickerDlg().open) return;
    const t = e.target as Element;
    if (t.closest(".screen-head, .screen-items")) return;
    const dest = pickerDest(t);
    if (!dest) return;
    cancelLongPress();
    const timer = window.setTimeout(() => {
      longPress = null;
      swallowClick = true;
      openPicker(dest);
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
    const t = e.target as Element;
    if (t.closest("[data-hold], [data-ffforce]")) return;
    const dest = pickerDest(t);
    if (!dest) return;
    e.preventDefault();
    if (lastPointer === "mouse" && !pickerDlg().open) openPicker(dest);
  });
  const dlg = pickerDlg();
  dlg.addEventListener("click", (e) => {
    const t = e.target as Element;
    if (t.closest("[data-close-picker]") || t === dlg) {
      dlg.close();
      pickFor = null;
      return;
    }
    const b = t.closest<HTMLElement>("[data-code], [data-wallet]");
    if (b) pick(b.dataset.code, b.dataset.wallet);
  });
}

boot().catch((e) => {
  app.innerHTML = `<p class="error">読み込みに失敗しました：${esc(e)}</p>`;
});
