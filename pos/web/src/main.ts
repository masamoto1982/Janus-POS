// 画面の組み立てとイベント処理。
//
// - 状態はすべて Rust コア（WASM）にあり、ここは「コマンドを送る」「スナップショットを描く」だけ。
// - 描き直しは領域ごとに、内容が変わったときだけ行う（問題1：無駄な再描画で操作が重くならない）。
//   入力中の値とフォーカスは描き直しの前後で引き継ぐ。
// - 確認ダイアログは使わない。結果はその画面に数秒だけ出るお知らせで返す（問題4）。
// - 買い物は、右の売り場から商品をレジの画面へドラッグ＆ドロップして再現する。
//   店員画面に落とせば店員側スキャナー、お客様画面に落とせばお客様側スキャナーで読んだことになる。

import init, { PosSimulator } from "./wasm/generated/janus_pos.js";
import type { Command, Side, Snapshot } from "./types";
import type { GuideKind, RulesTab } from "./view";
import { beepError, beepScan, setMatRinging, unlockAudio } from "./sound";
// 仮のロゴマーク（正式なロゴは後日差し替え）
import logoUrl from "./logo.jpg";
import {
  esc,
  FF_TABS,
  editing,
  ffVerifying,
  ffvInput,
  flashing,
  type FfUi,
  itemsPanel,
  screenHead,
  isoDate,
  scanGuide,
  voidAsk,
  collected,
  matPanel,
  bellSettings,
  ownerPanel,
  shelfPanel,
  walletPanel,
  sideOf,
  sharedScreen,
} from "./view";

declare global {
  interface Window {
    /** 単一 HTML 版でだけ埋め込まれる WASM 本体（base64） */
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
  // Bell は据え付け端末 1 台、可搬性端末 1 台、事務所のストアコンピューター 1 台
  reset(2, 3);
  setupDrag();
  window.setInterval(frame, 100);
}

function reset(registers: number, mats: number): void {
  sim?.free();
  sim = new PosSimulator(Date.now() & 0xffff, registers, mats);
  t0 = performance.now();
  snap = JSON.parse(sim.snapshot()) as Snapshot;
  buildSkeleton();
  wireSwipe();
  // 閉じたら（Esc でも）認証を解除し、パスワードを消す
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

// ---------------------------------------------------------------------------
// 骨組み（一度だけ作る）
// ---------------------------------------------------------------------------

function buildSkeleton(): void {
  const regs = snap.registers
    .map(
      (r) => `<section class="register" data-register="${r.id}" aria-label="レジ${r.id}">
      <h2>レジ${r.id} <span class="lamp" data-lamp="${r.id}" role="img" aria-label="レジ${r.id}の呼び出しランプ"></span><span data-region="mode-${r.id}"></span>
        <label class="login-sel">店員
          <select data-login="${r.id}" aria-label="レジ${r.id}の店員ログイン">
            <option value="">未ログイン</option>
            ${snap.accounts.map((a) => `<option value="${esc(a.barcode)}">${esc(a.name)}${a.role === "owner" ? "（オーナー）" : ""}</option>`).join("")}
          </select>
        </label>
      </h2>
      <div class="swipe-tabs" role="tablist" aria-label="表示する画面（スワイプでも切り替わります）">
        <button type="button" role="tab" class="on" data-swipe-to="${r.id}:staff">店員側</button>
        <button type="button" role="tab" data-swipe-to="${r.id}:customer">お客様側</button>
      </div>
      <div class="screens" data-swipe="${r.id}">
        <div class="screen-wrap" data-wrap="${r.id}:staff">
          <div class="screen-cap">店員側<span class="drop-hint">ここに落とす＝店員側スキャナーで読む</span></div>
          <section class="screen staff-screen" data-drop="${r.id}:staff">
            <div class="screen-head" data-region="head-${r.id}-staff"></div>
            <div class="screen-main" data-region="staff-${r.id}"></div>
            <div class="screen-items" data-region="items-${r.id}-staff"></div>
          </section>
        </div>
        <div class="screen-wrap" data-wrap="${r.id}:customer">
          <div class="screen-cap">お客様側<span class="drop-hint">ここに落とす＝お客様側スキャナーで読む</span></div>
          <section class="screen customer-screen" data-drop="${r.id}:customer">
            <div class="screen-head" data-region="head-${r.id}-customer"></div>
            <div class="screen-main" data-region="cust-${r.id}"></div>
            <div class="screen-items" data-region="items-${r.id}-customer"></div>
          </section>
        </div>
      </div>
      <div class="money-io" aria-label="レジ${r.id}の投入口と排出口（お客様側だけにあります）">
        <div class="money-io-cust" data-cash-drop="${r.id}">
        <input class="inlet" type="text" inputmode="numeric" autocomplete="off" placeholder="入金" data-key="in-${r.id}" data-inlet="${r.id}" aria-label="レジ${r.id}の投入口（金額を入れてEnter）" />
        <button type="button" class="outlet" data-outlet="${r.id}" aria-label="レジ${r.id}の排出口（金額をタッチで取り出す）"><span data-region="outlet-${r.id}"></span></button>
        </div>
      </div>
    </section>`,
    )
    .join("");

  app.innerHTML = `
    <header class="top">
      <img class="logo" src="${logoUrl}" width="73" height="73" alt="Janus POS ロゴ" />
      <h1>Janus POS</h1>
      <span class="build-badge" title="ビルド番号">Build ${esc(String(import.meta.env.VITE_BUILD ?? "dev"))}</span>
    </header>
    <article class="floor">
      <div class="left">
        <section class="mat-area" aria-label="Bell（呼び鈴）">
          <h2>Bell</h2>
          <div class="mats" data-region="mats"></div>
          <button type="button" class="icon-btn bell-open" data-open-bell aria-haspopup="dialog" aria-label="Bell の設定" title="Bell の設定">⚙</button>
          <div class="toast" id="toast-store"></div>
        </section>
        ${regs}
      </div>
      <div class="right-col">
        <aside class="right" aria-label="Item">
          <h2><span class="tip" tabindex="0" data-tip="お買い物をするには、アイテムをレジエリアにドラッグ＆ドロップします" aria-label="Item：お買い物をするには、アイテムをレジエリアにドラッグ＆ドロップします">Item <span class="info-icon" aria-hidden="true">i</span></span> <button type="button" class="icon-btn rules-open" data-open-rules aria-haspopup="dialog" aria-label="商品ルール設定" title="商品ルール設定">⚙</button></h2>
          <div data-region="shelf"></div>
        </aside>
        <aside class="right wallet" aria-label="Wallet">
          <h2>Wallet</h2>
          <div data-region="wallet"></div>
        </aside>
      </div>
    </article>
    <footer class="bottom">
      <p class="copyright"><a href="https://github.com/masamoto1982" target="_blank" rel="noopener">© 2026 masamoto yamashiro</a></p>
    </footer>
    <dialog class="rules-dlg" id="rules-dlg" aria-labelledby="rules-title">
      <div class="dlg-head"><h2 id="rules-title">商品ルール設定</h2><button class="small ghost" data-close-rules>閉じる</button></div>
      <form class="owner-auth" data-owner-auth>
        <p class="muted">商品ルールを変えられるのはオーナーだけです。オーナーの従業員番号とパスワードを入れてください。閉じると認証は解除されます。</p>
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
      <p class="muted">可搬性端末とストアコンピューターは、呼び鈴機能を OFF にできます。OFF の端末は鳴らず、不通の警告も出しません。据え付け端末は常に ON です。</p>
      <div class="bell-settings" data-region="bell-settings"></div>
    </dialog>
    <dialog class="rules-dlg picker-dlg" id="picker-dlg" aria-labelledby="picker-title">
      <div class="dlg-head"><h2 id="picker-title">選ぶ</h2><button class="small ghost" data-close-picker>閉じる</button></div>
      <div id="picker-body"></div>
    </dialog>
    <div class="drag-ghost" id="ghost" hidden></div>
    ${rampFilter()}`;
}

/** 操作できる画面の配色：グレーで描いた画面の濃淡を、そのまま青の濃淡に置き換える。
 *  白と黒はそのまま。4 段階のグレーは明るさをそろえた青（OKLab の明度をそろえ、P型・D型・T型の色覚でも灰色と見分けられる色相）になるので、文字のコントラストは変わらない */
const RAMP_STEPS: [number, string][] = [
  [0x00, "#000000"],
  [0x53, "#03578b"],
  [0x87, "#188edb"],
  [0xae, "#5bb6fe"],
  [0xd4, "#afdaff"],
  [0xff, "#ffffff"],
];

function rampFilter(): string {
  // グレーの値（0〜255）ごとに、隣り合う段のあいだを直線で結んだ表をつくる
  const table = (ch: number) =>
    Array.from({ length: 256 }, (_, v) => {
      const i = RAMP_STEPS.findIndex(([g]) => g >= v);
      const [g1, c1] = RAMP_STEPS[i];
      if (g1 === v || i === 0) return (parseInt(c1.slice(1 + ch * 2, 3 + ch * 2), 16) / 255).toFixed(4);
      const [g0, c0] = RAMP_STEPS[i - 1];
      const a = parseInt(c0.slice(1 + ch * 2, 3 + ch * 2), 16);
      const b = parseInt(c1.slice(1 + ch * 2, 3 + ch * 2), 16);
      return ((a + ((b - a) * (v - g0)) / (g1 - g0)) / 255).toFixed(4);
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

// ---------------------------------------------------------------------------
// 描画
// ---------------------------------------------------------------------------

function setRegion(name: string, html: string): void {
  const el = app.querySelector(`[data-region="${name}"]`);
  if (!el || cache.get(el) === html) return;
  cache.set(el, html);
  // 入力中の値とフォーカスを引き継ぐ
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

/** 登録した行を一瞬目立たせる時間 */
const FLASH_MS = 900;
/** 明細ごとの最後に見た数量（レジ:明細番号 → 数量）。新しい行や数量の変わった行を光らせる */
const seenQty = new Map<string, number>();
const flashUntil = new Map<string, number>();

/** 新しい動きがあったレジ（登録画面を一番上まで戻して、動いた行を見せる） */
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
  // 取引が終わると明細番号は振り直されるので、消えた行は忘れる
  for (const k of seenQty.keys()) if (!present.has(k)) seenQty.delete(k);
  flashing.clear();
  for (const [k, t] of flashUntil) {
    if (t <= now || !present.has(k)) flashUntil.delete(k);
    else flashing.add(k);
  }
}

function render(): void {
  trackFlashes();
  // 明細が表示エリアをはみ出してスクロールしていても、動きのあった行（一番上）が見えるように戻す
  for (const reg of movedRegs) {
    app.querySelectorAll<HTMLElement>(`[data-drop^="${reg}:"] .screen-main`).forEach((el) => (el.scrollTop = 0));
  }
  movedRegs.clear();
  for (const r of snap.registers) {
    // 店員ログインのセレクタは、操作中でなければ現在のログイン状態に合わせる
    const sel = app.querySelector<HTMLSelectElement>(`[data-login="${r.id}"]`);
    if (sel && document.activeElement !== sel) {
      const acc = r.staff.state === "absent" ? undefined : snap.accounts.find((a) => a.name === r.staff.name);
      sel.value = acc?.barcode ?? "";
    }
    // 呼び出しランプ：店員が来るまで点灯し、放置が長いほど速く点滅する
    const lamp = app.querySelector<HTMLElement>(`[data-lamp="${r.id}"]`);
    if (lamp) {
      const on = r.lamp;
      lamp.classList.toggle("on", on);
      lamp.style.setProperty("--blink", `${(1.4 - 1.1 * r.ring_volume).toFixed(2)}s`);
      lamp.title = on ? `呼び出し中（${Math.floor((r.call_waiting_ms ?? 0) / 1000)}秒）` : "呼び出しランプ";
    }
    // もう一方が触れている一瞬（またはお客様側に限られる場面）は、この画面の操作が利かない
    for (const side of ["staff", "customer"] as const) {
      const items = itemsPanel(r, snap, ffUi(r.id), side);
      const locked = r.active_side !== null && r.active_side !== side;
      const el = app.querySelector(`[data-drop="${r.id}:${side}"]`);
      el?.classList.toggle("inactive", locked);
      // 色を載せるのは操作できる面だけ。有人モードのお客様側は見るだけ（押せるのはお客様本人のボタンだけ）なのでグレー
      const operable = r.active_side !== null ? r.active_side === side : r.mode !== "attended" || side === "staff";
      el?.classList.toggle("operable", operable);
      // モバイルの切り替えタブも、操作できる面のタブだけ青にする
      app.querySelector(`[data-swipe-to="${r.id}:${side}"]`)?.classList.toggle("operable", operable);
      const wrap = el?.parentElement;
      wrap?.classList.toggle("inactive", locked);
      // レジ画面のヘッダー（稼働モードのバッジ・時刻・ベル。有人モードの店員側ではバッジが店舗メニューを開く）
      setRegion(`head-${r.id}-${side}`, screenHead(r, new Date(), side));
      setRegion(`items-${r.id}-${side}`, items);
      el?.classList.toggle("no-items", !items);
    }
    // モバイルモード：操作する面（アクティブ面）が変わったら、その面を表にする。
    // セミセルフはお客様側、有人は店員側、店舗メニューは開いた側（パスワード入力中はお客様側）。
    // 有人モードで触れている一瞬だけ変わる操作側には付いていかない。表にした後もスワイプで切り替えられる
    const face: Side = r.menu?.side ?? (r.menu_code ? "customer" : r.mode === "attended" ? "staff" : "customer");
    if (face !== activeShown.get(r.id)) {
      activeShown.set(r.id, face);
      if (isMobile()) swipeTo(r.id, face);
    }
    const idle = r.idle_revert_in;
    setRegion(
      `mode-${r.id}`,
      `<span class="mode mode-${r.mode}">${esc(r.mode_label)}</span>${
        idle !== null
          ? `<span class="idle" title="5分間操作がないと、登録中の取引も中止してセミセルフに戻ります">無操作であと ${Math.floor(idle / 60000)}:${String(Math.floor((idle % 60000) / 1000)).padStart(2, "0")} で${
              r.idle_voids ? (r.mode === "semi_self" ? "取引を中止します" : "取引を中止してセミセルフに戻ります") : "セミセルフに戻ります"
            }</span>`
          : ""
      }`,
    );
    // 両方の画面に同じ内容を出す（呼び出しの表示だけは側ごとに出し分ける）
    setRegion(`staff-${r.id}`, sharedScreen(r, snap, "staff"));
    setRegion(`cust-${r.id}`, sharedScreen(r, snap, "customer"));
    setRegion(
      `outlet-${r.id}`,
      r.outlet ? `<b title="${r.outlet_detail.map(([d, n]) => `${d.toLocaleString()}円×${n}`).join(" ")}">${r.outlet.toLocaleString()}円</b>` : `<span class="placeholder">出金</span>`,
    );
    // おつりなど、出金ボックスにお金があれば枠と文字を点滅させる
    app.querySelector(`[data-outlet="${r.id}"]`)?.classList.toggle("blink", r.outlet > 0);
    // 入金ボックス：支払方法が決まったら中の文字を点滅させる。現金は投入額を積み上げて見せる
    const inlet = app.querySelector<HTMLInputElement>(`[data-inlet="${r.id}"]`);
    if (inlet) {
      const paying = !r.menu && r.staff.phase === "paying" && r.staff.method !== null && r.emoney_short === null && r.emoney_partial === null;
      // 電子マネーのチャージ・残高確認の間は、その手続きの案内を出す
      const emoney = r.menu ? null : r.emoney;
      // 「不足分を現金で払う」では、案内を確認したあとで電子マネーをかざしてもらう
      const partial = !r.menu && r.emoney_partial !== null && r.emoney_partial_ok;
      inlet.classList.toggle("blink", paying || partial || (!!emoney && emoney.kind !== "menu"));
      // 中の文字は、選んだ支払方法に合わせて何をすればよいかを示す
      inlet.placeholder = emoney && emoney.kind !== "menu"
        ? emoney.kind === "charge"
          ? emoney.inserted
            ? "電子マネーをかざす"
            : "紙幣を入れる"
          : "電子マネーをかざす"
        : partial
          ? "電子マネーをかざす"
          : paying
          ? (INLET_HINT[r.staff.method!] ?? "入金")
          : "入金";
      const shown = emoney ? emoney.inserted : !r.menu ? r.staff.inserted : 0;
      if (!inlet.dataset.typed) inlet.value = shown ? String(shown) : "";
    }
  }
  setRegion("mats", matPanel(snap));
  setRegion("bell-settings", bellSettings(snap));
  setRegion("owner", ownerAuth ? ownerPanel(snap, rulesTab) : "");
  setRegion("shelf", shelfPanel(snap));
  setRegion("wallet", walletPanel(snap.emoney_balance));
}


// FF タブとページ（画面の表示だけの状態なので、レジごとに UI 側で持つ）
const ffTabs = new Map<number, FfUi>();
const ffUi = (reg: number): FfUi => ffTabs.get(reg) ?? { tab: FF_TABS[0][0], page: 0 };

function frame(): void {
  sim.tick(performance.now() - t0);
  snap = JSON.parse(sim.snapshot()) as Snapshot;
  // MAT が鳴動中の間だけ鳴らす（お客様が呼び出しボタンに触れている間。順番待ちにはしない）。
  // 音量は最も長く待たせている呼び出しで決まる
  setMatRinging(
    snap.mats.some((m) => m.buzzing),
    Math.max(0, ...snap.registers.map((r) => r.ring_volume)),
  );
  render();
  app.querySelectorAll<HTMLElement>("[data-hold]").forEach((b) => b.classList.toggle("pressing", b.dataset.hold === held?.cmd));
  app.querySelectorAll<HTMLElement>("[data-hold-confirm]").forEach((b) => b.classList.toggle("holding", b.dataset.holdConfirm === confirming?.cmd));
}

// ---------------------------------------------------------------------------
// 長押しで決まるボタン（取引中止）：押し続けて HOLD_MS たつと送る。途中で離す・指が外れると取り消し
// ---------------------------------------------------------------------------

const HOLD_MS = 1500;
let confirming: { cmd: string; pointer: number | null; timer: number } | null = null;

function startConfirm(b: HTMLElement, pointer: number | null): void {
  if (confirming || (b as HTMLButtonElement).disabled) return;
  const cmd = b.dataset.holdConfirm!;
  const timer = window.setTimeout(() => {
    confirming = null;
    const c = JSON.parse(cmd) as Command;
    voidAsk.delete(c.register as number);
    send(c);
  }, HOLD_MS);
  confirming = { cmd, pointer, timer };
  frame();
}

function cancelConfirm(): void {
  if (!confirming) return;
  window.clearTimeout(confirming.timer);
  confirming = null;
  frame();
}

// ---------------------------------------------------------------------------
// 呼び出しボタン：触れている間だけ鳴る。離す・指が外れる・画面を離れると止まる
// ---------------------------------------------------------------------------

const activeShown = new Map<number, Side>();

let held: { cmd: string; register: number; pointer: number | null } | null = null;

function press(b: HTMLElement, pointer: number | null): void {
  if (held || (b as HTMLButtonElement).disabled) return;
  const cmd = b.dataset.hold!;
  const c = JSON.parse(cmd) as Command;
  if (send(c)) held = { cmd, register: c.register as number, pointer };
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

function send(cmd: Command): boolean {
  sim.tick(performance.now() - t0);
  const res = JSON.parse(sim.dispatch(JSON.stringify(cmd))) as { ok: boolean; message?: string; error?: string };
  // レジ画面には覆い隠すメッセージを出さない（登録した行が一瞬色を変えて知らせる）。
  // 知らせを出すのは、レジ画面の外（年確ルールの設定・店舗全体の操作）だけ
  const where = RULE_CMDS.includes(cmd.type) ? "toast-rules" : sideOf(cmd) === "store" ? "toast-store" : null;
  if (where) {
    if (res.ok) {
      if (res.message) toast(where, res.message, false);
    } else toast(where, res.error ?? "操作できません", true);
  }
  if (!res.ok) {
    // 買い物点数の上限は表示エリアに合わせたシミュレーターだけの制約（実際のシステムでは起きない）
    if (res.error?.includes("買い物点数の上限")) setTimeout(() => alert(res.error), 0);
    // 販売個数の上限はオーナーが決めた売り方なので、登録できない理由をその場で知らせる
    if (res.error?.includes("1回のお会計で")) setTimeout(() => alert(res.error), 0);
  }
  // スキャナーと FF 登録の音（読めたら「ピッ」、受け付けられなければ「ブッブー」）
  if (["scan", "register_ff", "tap_emoney"].includes(cmd.type)) (res.ok ? beepScan : beepError)();
  frame();
  return res.ok;
}

// ---------------------------------------------------------------------------
// イベント（委譲。描き直しでボタンが入れ替わっても取りこぼさない）
// ---------------------------------------------------------------------------

// 使い方の説明：Item・Wallet のボタンにカーソルを重ねると、少し遅れて出す（画面の外にはみ出さない位置に置く）
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

// 両方の画面に同じ入力欄があるので、押したボタンのある画面（操作している側）の欄を読む
let scope: ParentNode = app;
const val = (key: string): string => scope.querySelector<HTMLInputElement>(`[data-key="${key}"]`)?.value.trim() ?? "";
const clear = (key: string): void => {
  const el = scope.querySelector<HTMLInputElement>(`[data-key="${key}"]`);
  if (el) el.value = "";
};

/** 全角の数字・小数点を半角に */
const han = (s: string): string => s.replace(/[０-９．]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xfee0));

function form(b: HTMLElement): void {
  const reg = Number(b.dataset.reg);
  scope = b.closest("[data-drop]") ?? app;
  switch (b.dataset.form) {
    case "service":
      send({ type: "complete_service", register: reg, line_id: Number(b.dataset.line), amount: Number(val(`svc-${reg}-${b.dataset.line}`) || 0) });
      break;
    case "parcel-length":
      send({ type: "parcel_length", register: reg, line_id: Number(b.dataset.line), sum_cm: Math.round(Number(han(val(`pcm-${reg}`)) || 0)) });
      break;
    case "parcel-size":
      send({
        type: "parcel_size",
        register: reg,
        line_id: Number(b.dataset.line),
        sum_cm: Number(b.dataset.cm),
        weight_g: Math.round(Number(han(val(`pkg-${reg}`)) || 0) * 1000),
      });
      break;
    case "parcel-postal":
      send({ type: "parcel_postal", register: reg, line_id: Number(b.dataset.line), postal: val(`ppost-${reg}`) });
      break;
    case "birth":
      send({ type: "check_id", register: reg, side: "staff", birth: val(`birth-${reg}`), today: isoDate(new Date()) });
      break;
    case "cashout":
      send({
        type: "cash_out",
        register: reg,
        kind: b.dataset.kind,
        amount: Number(val(`mo-amt-${reg}`) || 0),
      });
      clear(`mo-amt-${reg}`);
      break;
    case "menucode":
      send({ type: "open_menu", register: reg, side: "customer", screen: "top", code: val(`mc-${reg}`) });
      clear(`mc-${reg}`);
      break;
  }
}

// 投入口：数字だけを受け付け、Enter で入れる
app.addEventListener("input", (e) => {
  const el = e.target as HTMLInputElement;
  if (el.dataset.inlet) {
    el.value = el.value.replace(/[^0-9０-９]/g, "");
    el.dataset.typed = "1";
  }
  // 生年月日：西暦 8 桁。全角の数字は半角に直す
  // 店舗メニューのパスワード：英大文字と数字の 4 桁（小文字・全角は直す）
  if (el.dataset.menucode)
    el.value = el.value
      .replace(/[０-９Ａ-Ｚａ-ｚ]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xfee0))
      .toUpperCase()
      .replace(/[^A-Z0-9]/g, "")
      .slice(0, 4);
  // お渡し照合：お渡し数を入力するたびに判定する（判定ボタンはない）。全角の数字は半角に直す
  if (el.dataset.ffvLine) {
    el.value = el.value.replace(/[０-９]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xfee0)).replace(/[^0-9]/g, "").slice(0, 3);
    const [reg, side, line] = el.dataset.ffvLine.split(":");
    ffvInput.set(el.dataset.ffvLine, el.value);
    send({ type: "check_ff", register: Number(reg), side, line_id: Number(line), handed: Number(el.value || 0) });
    return;
  }
  // 払込票の確認：事業者・金額が合っていれば一枚ずつチェックを入れる（外すこともできる）
  if (el.dataset.billCheck) {
    const [reg, side, line] = el.dataset.billCheck.split(":");
    send({ type: "check_bill", register: Number(reg), side, line_id: Number(line), checked: el.checked });
    return;
  }
  if (el.dataset.birth) el.value = el.value.replace(/[０-９]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xfee0)).replace(/[^0-9]/g, "").slice(0, 8);
});
app.addEventListener("keydown", (e) => {
  const el = e.target as HTMLInputElement;
  if (!el.dataset?.menucode || e.key !== "Enter" || e.isComposing) return;
  el.closest(".menu-code")?.querySelector<HTMLButtonElement>('[data-form="menucode"]')?.click();
});
// 荷物のサイズ・郵便番号の入力欄では Enter で決定
app.addEventListener("keydown", (e) => {
  const el = e.target as HTMLInputElement;
  if (!el.dataset?.parcel || e.key !== "Enter" || e.isComposing) return;
  const box = el.closest(".parcel-entry");
  const inputs = [...(box?.querySelectorAll<HTMLInputElement>("input[data-parcel]") ?? [])];
  const next = inputs[inputs.indexOf(el) + 1];
  if (next) next.focus();
  else box?.querySelector<HTMLButtonElement>('[data-form^="parcel-"]')?.click();
});
app.addEventListener("keydown", (e) => {
  const el = e.target as HTMLInputElement;
  if (!el.dataset?.birth || e.key !== "Enter" || e.isComposing) return;
  el.closest(".id-entry")?.querySelector<HTMLButtonElement>('[data-form="birth"]')?.click();
});
app.addEventListener("keydown", (e) => {
  const el = e.target as HTMLInputElement;
  if (!el.dataset?.inlet || e.key !== "Enter" || e.isComposing) return;
  const amount = Number(el.value.replace(/[０-９]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xfee0)) || 0);
  const register = Number(el.dataset.inlet);
  const r = snap.registers.find((x) => x.id === register);
  delete el.dataset.typed;
  // 店舗メニューの入金・両替は、打ち込んだ金額をそのまま入れる
  if (r?.menu) {
    if (send({ type: "insert", register, amount })) el.value = "";
    return;
  }
  // お会計：箱の金額が投入額。打ち込んで増やした分を入れてから、Enter で投入を終えて決済する
  const extra = amount - (r?.staff.inserted ?? 0);
  if (extra > 0 && !send({ type: "insert", register, amount: extra })) return;
  send({ type: "finish_insert", register });
});

app.addEventListener("change", (e) => {
  const sel = e.target as HTMLSelectElement;
  if (sel.dataset.bellOn) {
    const box = sel as unknown as HTMLInputElement;
    if (!send({ type: "set_bell_on", mat: Number(box.dataset.bellOn), on: box.checked })) box.checked = !box.checked;
    return;
  }
  if (sel.dataset.collect) {
    // 回収リストのチェック。画面の中だけで持ち、全部入ると「回収しました」が押せる
    const box = sel as unknown as HTMLInputElement;
    const [reg, i] = box.dataset.collect!.split(":").map(Number);
    const set = collected.get(reg) ?? new Set<number>();
    if (box.checked) set.add(i);
    else set.delete(i);
    collected.set(reg, set);
    frame();
    return;
  }
  if (sel.dataset.rulesTab !== undefined) {
    rulesTab = sel.value as RulesTab;
    render();
    return;
  }
  if (sel.dataset.taxRate && ownerAuth) {
    const rate = Number((sel as unknown as HTMLInputElement).value);
    const ok = Number.isFinite(rate) && send({ type: "set_tax_rate", ...ownerAuth, class_id: sel.dataset.taxRate, rate_bp: Math.round(rate * 100) });
    // 認められなかったら表示を元の税率に戻す
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
    // 認められなかったら表示を元の設定に戻す
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

// 商品ルール設定：開くときにオーナー認証し、閉じるまでの変更はその認証で送る（コアは操作のたびにも検証する）
const RULE_CMDS = ["verify_owner", "set_product_age", "set_sale_limit", "set_tax_rate", "add_tax_class", "set_product_tax", "set_invoice"];
let ownerAuth: { owner_id: string; password: string } | null = null;
let rulesTab: RulesTab = "age";

function lockRules(): void {
  ownerAuth = null;
  const pw = rulesDlg().querySelector<HTMLInputElement>(`[data-owner="pw"]`);
  if (pw) pw.value = "";
  const form = rulesDlg().querySelector<HTMLElement>("[data-owner-auth]");
  if (form) form.hidden = false;
  render();
}

// ブラウザは操作があるまで音を出せないので、最初のタッチ・キー入力で音を準備する
document.addEventListener("pointerdown", unlockAudio, { capture: true });
document.addEventListener("keydown", unlockAudio, { capture: true });
// iOS は pointerdown では音を出す準備ができないので、指を離したときにも準備する
document.addEventListener("touchend", unlockAudio, { capture: true });

app.addEventListener("click", (e) => {
  const b = (e.target as Element).closest("button");
  if (!b || b.disabled) return;
  if (b.hasAttribute("data-open-rules")) return rulesDlg().showModal();
  if (b.hasAttribute("data-close-rules")) return rulesDlg().close();
  if (b.hasAttribute("data-add-tax") && ownerAuth) {
    const field = (k: string) => rulesDlg().querySelector<HTMLInputElement>(`[data-new-tax="${k}"]`)!;
    const rate = Number(field("rate").value || 0);
    if (
      send({ type: "add_tax_class", ...ownerAuth, name: field("name").value, kind: field("kind").value, rate_bp: Math.round(rate * 100) })
    ) {
      field("name").value = "";
      field("rate").value = "";
    }
    return;
  }
  if (b.hasAttribute("data-save-invoice") && ownerAuth) {
    const field = (k: string) => rulesDlg().querySelector<HTMLInputElement>(`[data-invoice="${k}"]`)!;
    send({ type: "set_invoice", ...ownerAuth, invoice_no: field("no").value, rounding: field("rounding").value });
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
  // 稼働モードのバッジ：開いた側の画面で店舗メニューを開く・閉じる。
  // お客様側は、店員画面に出る使い捨てパスワードを入力して開く
  if (b.dataset.gear) {
    const [reg, side] = b.dataset.gear.split(":") as [string, Side];
    const r = snap.registers.find((x) => x.id === Number(reg));
    const open = r?.menu?.side === side || (side === "customer" && !!r?.menu_code);
    if (open) send({ type: "open_menu", register: Number(reg), side, screen: null });
    else if (side === "customer") send({ type: "request_menu_code", register: Number(reg) });
    else send({ type: "open_menu", register: Number(reg), side, screen: "top" });
    return;
  }
  if (b.dataset.outlet) {
    send({ type: "take_outlet", register: Number(b.dataset.outlet) });
    return;
  }
  // 取引中止：確認画面を開く・閉じる（決めるのは確認画面の長押しだけ）
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
  if (b.dataset.guide) {
    // 初期画面の案内ボタン：もう一度押すと閉じる
    const [reg, side, kind] = b.dataset.guide.split(":");
    const key = `${reg}:${side}`;
    if (scanGuide.get(key) === kind) scanGuide.delete(key);
    else scanGuide.set(key, kind as GuideKind);
    frame();
    return;
  }
  // FF の「お渡し前」：お渡し照合の画面を開く（登録画面を置き換える）
  if (b.dataset.ffverify) {
    ffVerifying.add(b.dataset.ffverify);
    editing.delete(b.dataset.ffverify);
    frame();
    return;
  }
  // 商品登録に戻る（照合の途中でも）。入力した数は消す
  if (b.dataset.ffverifyClose) {
    ffVerifying.delete(b.dataset.ffverifyClose);
    for (const k of [...ffvInput.keys()]) if (k.startsWith(`${b.dataset.ffverifyClose}:`)) ffvInput.delete(k);
    frame();
    return;
  }
  // 明細の数量ボタン：行を広げて数量変更・取り消し（登録画面の中で完結。コアには決めたときだけ送る）
  if (b.dataset.edit) {
    const [reg, side, id, qty] = b.dataset.edit.split(":");
    const key = `${reg}:${side}`;
    if (editing.get(key)?.id === Number(id)) editing.delete(key);
    else editing.set(key, { id: Number(id), qty: Number(qty) });
    frame();
    return;
  }
  if (b.dataset.editStep) {
    const [reg, side, d] = b.dataset.editStep.split(":");
    const e = editing.get(`${reg}:${side}`);
    if (e) e.qty = Math.max(1, e.qty + Number(d));
    frame();
    return;
  }
  if (b.dataset.editClose) {
    editing.delete(b.dataset.editClose);
    frame();
    return;
  }
  if (b.dataset.cmd) {
    const cmd = JSON.parse(b.dataset.cmd) as Command;
    // 個数変更・取り消しが済んだら、広げた行を閉じる
    if (send(cmd) && ["set_qty", "remove_line", "remove_coupon"].includes(cmd.type)) editing.delete(`${cmd.register}:${cmd.side}`);
  }
  else if (b.dataset.form) form(b);
});

// ---------------------------------------------------------------------------
// ドラッグ＆ドロップ（ポインターイベントで実装。マウスでもタッチでも同じに動く）
// ---------------------------------------------------------------------------

// 入金ボックスの中の文字（支払方法ごと）
const INLET_HINT: Record<string, string> = {
  cash: "現金を入れる",
  emoney: "電子マネーをかざす",
  qr: "バーコードをかざす",
  credit: "カードをかざす",
  points: "ポイントをかざす",
};

function setupDrag(): void {
  const ghost = document.getElementById("ghost")!;
  // code：レジで読むもの（商品・ポイントカード）、wallet：お金（cash:金額）とかざす決済（pay:方法）
  let drag: { code?: string; wallet?: string; label: string; x: number; y: number; moved: boolean; id: number } | null = null;

  const target = (x: number, y: number): Element | null => document.elementFromPoint(x, y)?.closest("[data-drop], [data-cash-drop]") ?? null;
  const end = () => {
    drag = null;
    ghost.hidden = true;
    document.body.classList.remove("dragging", "dragging-wallet");
  };

  app.addEventListener("pointerdown", (e) => {
    const b = (e.target as Element).closest<HTMLElement>("[data-code], [data-wallet]");
    if (!b || e.button !== 0 || b.closest("#picker-dlg")) return;
    e.preventDefault();
    drag = { code: b.dataset.code, wallet: b.dataset.wallet, label: b.querySelector(".item-name")?.textContent ?? "", x: e.clientX, y: e.clientY, moved: false, id: e.pointerId };
  });

  window.addEventListener("pointermove", (e) => {
    if (!drag || e.pointerId !== drag.id) return;
    if (!drag.moved && Math.hypot(e.clientX - drag.x, e.clientY - drag.y) < 5) return;
    if (!drag.moved) {
      drag.moved = true;
      ghost.textContent = drag.label;
      ghost.hidden = false;
      document.body.classList.add("dragging");
      // お金・決済は入金ボックスにも落とせる
      document.body.classList.toggle("dragging-wallet", !!drag.wallet);
    }
    ghost.style.transform = `translate(${e.clientX + 8}px, ${e.clientY + 8}px)`;
  });

  window.addEventListener("pointerup", (e) => {
    if (!drag || e.pointerId !== drag.id) return;
    const d = drag;
    const t = target(e.clientX, e.clientY);
    end();
    if (!d.moved) {
      toast("toast-store", "ドラッグして、レジの店員画面・お客様画面（お金は入金ボックスにも）に落としてください", false);
      return;
    }
    if (!t) return;
    const el = t as HTMLElement;
    // 入金ボックスの周り（お客様側）に落としたときは、お客様側として扱う
    const [reg, side] = el.dataset.drop ? el.dataset.drop.split(":") : [el.dataset.cashDrop!, "customer"];
    const register = Number(reg);
    // ポイント利用を選んだあとは、ポイントカード・アプリを入金ボックスにかざすと決済になる
    const pointsChosen = snap.registers.find((r) => r.id === register)?.customer.method === "points";
    if (d.code?.startsWith("PC") && el.dataset.cashDrop && pointsChosen) return void send({ type: "tap_emoney", register, method: "points" });
    if (d.code) return void send({ type: "scan", register, side, code: d.code });
    const [kind, value] = d.wallet!.split(":");
    // お金は投入口（お客様側にしかない）へ、決済はお客様側の端末にかざす
    if (kind === "cash") send({ type: "insert", register, amount: Number(value) });
    else send({ type: "tap_emoney", register, method: value });
  });

  window.addEventListener("pointercancel", end);

  app.addEventListener("pointerdown", (e) => {
    const b = (e.target as Element).closest<HTMLElement>("[data-hold]");
    if (!b || e.button !== 0) return;
    e.preventDefault();
    press(b, e.pointerId);
  });
  app.addEventListener("pointerdown", (e) => {
    const b = (e.target as Element).closest<HTMLElement>("[data-hold-confirm]");
    if (!b || e.button !== 0) return;
    e.preventDefault();
    startConfirm(b, e.pointerId);
  });
  window.addEventListener("pointermove", (e) => {
    if (!confirming || e.pointerId !== confirming.pointer) return;
    const over = document.elementFromPoint(e.clientX, e.clientY)?.closest<HTMLElement>("[data-hold-confirm]");
    if (over?.dataset.holdConfirm !== confirming.cmd) cancelConfirm();
  });
  for (const ev of ["pointerup", "pointercancel"] as const) {
    window.addEventListener(ev, (e) => {
      if (confirming && e.pointerId === confirming.pointer) cancelConfirm();
    });
  }
  window.addEventListener("blur", cancelConfirm);
  app.addEventListener("keydown", (e) => {
    const b = (e.target as Element).closest<HTMLElement>("[data-hold-confirm]");
    if (!b || e.repeat || (e.key !== " " && e.key !== "Enter")) return;
    e.preventDefault();
    startConfirm(b, null);
  });
  app.addEventListener("keyup", (e) => {
    if (confirming && confirming.pointer === null && (e.key === " " || e.key === "Enter")) cancelConfirm();
  });
  window.addEventListener("pointermove", (e) => {
    if (!held || e.pointerId !== held.pointer) return;
    // 指がボタンから外れたら離したのと同じ
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
    if ((e.target as Element).closest("[data-hold], [data-hold-confirm]")) e.preventDefault();
  });
  // キーボード：Space / Enter を押している間
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

// ---------------------------------------------------------------------------
// モバイルモード（画面の小さい端末）：Item・Wallet の欄は出さず、レジ画面を長押しすると Item の商品を、
// 入金ボックスを長押しすると Wallet の決済方法（お金・カード）を選べる。選んだものは、その画面のスキャナー
// （お金・決済はそのレジの入金ボックス）で読む。店員側とお客様側の画面はスワイプで切り替える
// ---------------------------------------------------------------------------

const MOBILE = window.matchMedia("(max-width: 760px)");
const isMobile = (): boolean => MOBILE.matches;
/** 長押しとみなす時間 */
const LONG_PRESS_MS = 500;
let longPress: { timer: number; x: number; y: number; id: number } | null = null;
/** 長押しのあとの指を離したクリックは、下にあるボタンに届けない */
let swallowClick = false;
/** 長押しで開いた選択画面の行き先 */
let pickFor: { register: number; side: Side; kind: "item" | "wallet" } | null = null;

const pickerDlg = (): HTMLDialogElement => document.getElementById("picker-dlg") as HTMLDialogElement;

function openPicker(register: number, side: Side, kind: "item" | "wallet"): void {
  pickFor = { register, side, kind };
  const title = kind === "item" ? `レジ${register}・${side === "staff" ? "店員側" : "お客様側"}で読む商品` : `レジ${register}の入金ボックスに入れる・かざす`;
  document.getElementById("picker-title")!.textContent = title;
  const body = document.getElementById("picker-body")!;
  body.innerHTML = kind === "item" ? shelfPanel(snap) : walletPanel(snap.emoney_balance);
  pickerDlg().showModal();
}

function pick(code?: string, wallet?: string): void {
  if (!pickFor) return;
  const { register, side } = pickFor;
  pickerDlg().close();
  pickFor = null;
  const pointsChosen = snap.registers.find((r) => r.id === register)?.customer.method === "points";
  if (code?.startsWith("PC") && pointsChosen) return void send({ type: "tap_emoney", register, method: "points" });
  if (code) return void send({ type: "scan", register, side, code });
  const [kind, value] = wallet!.split(":");
  if (kind === "cash") send({ type: "insert", register, amount: Number(value) });
  else send({ type: "tap_emoney", register, method: value });
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
}

function cancelLongPress(): void {
  if (longPress) clearTimeout(longPress.timer);
  longPress = null;
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
  // 長押し：レジ画面なら Item、入金ボックスなら Wallet
  app.addEventListener("pointerdown", (e) => {
    if (!isMobile() || e.button !== 0 || pickerDlg().open) return;
    const t = e.target as Element;
    const inlet = t.closest<HTMLElement>("[data-inlet]");
    // 最上部（呼び鈴など）と最下段（FF・サービス・たばこ）は長押しで操作するボタンがあるので対象外
    const screen = t.closest(".screen-head, .screen-items") ? null : t.closest<HTMLElement>("[data-drop]");
    if (!inlet && !screen) return;
    cancelLongPress();
    const timer = window.setTimeout(() => {
      longPress = null;
      swallowClick = true;
      if (inlet) {
        (inlet as HTMLInputElement).blur();
        openPicker(Number(inlet.dataset.inlet), "customer", "wallet");
      } else {
        const [reg, side] = screen!.dataset.drop!.split(":");
        openPicker(Number(reg), side as Side, "item");
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
  // 長押しを離したときの click が出ない端末もあるので、次に触れた時点で打ち消しをやめる
  window.addEventListener("pointerdown", () => (swallowClick = false), true);
  app.addEventListener("contextmenu", (e) => {
    if (isMobile() && (e.target as Element).closest("[data-drop], [data-inlet]")) e.preventDefault();
  });
  // 選択画面：タップで選ぶ（指で縦にスクロールできるよう、ドラッグはしない）
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
