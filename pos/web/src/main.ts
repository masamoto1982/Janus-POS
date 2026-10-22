// 画面の組み立てとイベント処理。
//
// - 状態はすべて Rust コア（WASM）にあり、ここは「コマンドを送る」「スナップショットを描く」だけ。
// - 描き直しは領域ごとに、内容が変わったときだけ行う（問題1：無駄な再描画で操作が重くならない）。
//   入力中の値とフォーカスは描き直しの前後で引き継ぐ。
// - 確認ダイアログは使わない。結果はその画面に数秒だけ出るお知らせで返す（問題4）。
// - 買い物は、右の売り場から商品をレジの画面へドラッグ＆ドロップして再現する。
//   店員画面に落とせば店員側スキャナー、お客様画面に落とせばお客様側スキャナーで読んだことになる。

import init, { PosSimulator } from "./wasm/generated/janus_pos.js";
import type { Command, Snapshot } from "./types";
import type { GuideKind } from "./view";
import { beepError, beepScan, setMatRinging, unlockAudio } from "./sound";
import {
  esc,
  FF_TABS,
  editing,
  flashing,
  type FfUi,
  itemsPanel,
  screenHead,
  isoDate,
  scanGuide,
  collected,
  matPanel,
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
  // 呼び鈴は据え付け型 1 台と、携帯型の検品端末 1 台
  reset(2, 2);
  setupDrag();
  window.setInterval(frame, 100);
}

function reset(registers: number, mats: number): void {
  sim?.free();
  sim = new PosSimulator(Date.now() & 0xffff, registers, mats);
  t0 = performance.now();
  snap = JSON.parse(sim.snapshot()) as Snapshot;
  buildSkeleton();
  // 閉じたら（Esc でも）オーナーのパスワードを消す
  rulesDlg().addEventListener("close", () => {
    const pw = rulesDlg().querySelector<HTMLInputElement>(`[data-owner="pw"]`);
    if (pw) pw.value = "";
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
            ${snap.accounts.map((a) => `<option value="${esc(a.barcode)}">${esc(a.name)}</option>`).join("")}
          </select>
        </label>
      </h2>
      <div class="screens">
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
      <h1>Janus POS</h1>
      <p class="muted">右の Item から商品をつかんで、レジの店員画面かお客様画面に落とすと、その側のスキャナーで読んだことになります。</p>
    </header>
    <article class="floor">
      <div class="left">
        <section class="mat-area" aria-label="Bell（呼び鈴）">
          <h2>Bell</h2>
          <div class="mats" data-region="mats"></div>
          <div class="toast" id="toast-store"></div>
        </section>
        ${regs}
      </div>
      <div class="right-col">
        <aside class="right" aria-label="Item">
          <h2>Item <button type="button" class="icon-btn rules-open" data-open-rules aria-haspopup="dialog" aria-label="年確ルール設定" title="年確ルール設定">⚙</button></h2>
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
      <div class="dlg-head"><h2 id="rules-title">年確ルール設定</h2><button class="small ghost" data-close-rules>閉じる</button></div>
      <p class="muted">変えられるのはオーナーだけです。変更のたびにここのパスワードで認証し、閉じるとパスワードは消えます。酒・たばこは法令どおり 20歳以上で固定され、変更できません。年確は会計ボタンを押した時点で、カゴの中で最も厳しい年齢で1回だけ行います。</p>
      <div class="row wrap">
        <input data-owner="id" placeholder="オーナーの従業員番号" value="9001" aria-label="オーナーの従業員番号" />
        <input data-owner="pw" type="password" placeholder="パスワード" autocomplete="off" aria-label="オーナーのパスワード" />
      </div>
      <div class="toast" id="toast-rules"></div>
      <div data-region="owner"></div>
    </dialog>
    <div class="drag-ghost" id="ghost" hidden></div>`;
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
      const wrap = el?.parentElement;
      wrap?.classList.toggle("inactive", locked);
      // レジ画面のヘッダー（☰ は店員ログイン中の店員側だけ。時刻とベルは常時）
      setRegion(`head-${r.id}-${side}`, screenHead(r, new Date(), side));
      setRegion(`items-${r.id}-${side}`, items);
      el?.classList.toggle("no-items", !items);
    }
    const idle = r.idle_revert_in;
    setRegion(
      `mode-${r.id}`,
      `<span class="mode mode-${r.mode}">${esc(r.mode_label)}</span>${
        idle !== null ? `<span class="idle" title="5分間操作がないとセミセルフに戻って待機します">無操作であと ${Math.floor(idle / 60000)}:${String(Math.floor((idle % 60000) / 1000)).padStart(2, "0")} でセミセルフに戻ります</span>` : ""
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
      inlet.classList.toggle("blink", !r.menu && r.staff.phase === "paying" && r.staff.method !== null);
      if (!inlet.dataset.typed) inlet.value = !r.menu && r.staff.inserted ? String(r.staff.inserted) : "";
    }
  }
  setRegion("mats", matPanel(snap));
  setRegion("owner", ownerPanel(snap));
  setRegion("shelf", shelfPanel(snap));
  setRegion("wallet", walletPanel());
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
}

// ---------------------------------------------------------------------------
// 呼び出しボタン：触れている間だけ鳴る。離す・指が外れる・画面を離れると止まる
// ---------------------------------------------------------------------------

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
  const where = cmd.type === "set_product_age" ? "toast-rules" : sideOf(cmd) === "store" ? "toast-store" : null;
  if (where) {
    if (res.ok) {
      if (res.message) toast(where, res.message, false);
    } else toast(where, res.error ?? "操作できません", true);
  }
  if (!res.ok) {
    // 買い物点数の上限は表示エリアに合わせたシミュレーターだけの制約（実際のシステムでは起きない）
    if (res.error?.includes("買い物点数の上限")) setTimeout(() => alert(res.error), 0);
  }
  // スキャナーと FF 登録の音（読めたら「ピッ」、受け付けられなければ「ブッブー」）
  if (["scan", "register_ff", "tap_emoney"].includes(cmd.type)) (res.ok ? beepScan : beepError)();
  frame();
  return res.ok;
}

// ---------------------------------------------------------------------------
// イベント（委譲。描き直しでボタンが入れ替わっても取りこぼさない）
// ---------------------------------------------------------------------------

// 両方の画面に同じ入力欄があるので、押したボタンのある画面（操作している側）の欄を読む
let scope: ParentNode = app;
const val = (key: string): string => scope.querySelector<HTMLInputElement>(`[data-key="${key}"]`)?.value.trim() ?? "";
const clear = (key: string): void => {
  const el = scope.querySelector<HTMLInputElement>(`[data-key="${key}"]`);
  if (el) el.value = "";
};

function form(b: HTMLElement): void {
  const reg = Number(b.dataset.reg);
  scope = b.closest("[data-drop]") ?? app;
  switch (b.dataset.form) {
    case "service":
      send({ type: "complete_service", register: reg, line_id: Number(b.dataset.line), amount: Number(val(`svc-${reg}-${b.dataset.line}`) || 0) });
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
        password: val(`mo-pw-${reg}`),
      });
      clear(`mo-pw-${reg}`);
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
  if (el.dataset.birth) el.value = el.value.replace(/[０-９]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xfee0)).replace(/[^0-9]/g, "").slice(0, 8);
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
  if (sel.dataset.ffcheck) {
    // お渡し確認のチェック。押した画面の側からの操作になる
    const box = sel as unknown as HTMLInputElement;
    const [reg, line] = box.dataset.ffcheck!.split(":").map(Number);
    const side = box.closest<HTMLElement>("[data-drop]")?.dataset.drop?.split(":")[1] ?? "staff";
    if (!send({ type: "check_ff", register: reg, side, line_id: line, checked: box.checked })) box.checked = !box.checked;
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
  if (sel.dataset.age) {
    const owner = (k: string) => app.querySelector<HTMLInputElement>(`[data-owner="${k}"]`)?.value.trim() ?? "";
    const ok = send({
      type: "set_product_age",
      owner_id: owner("id"),
      password: owner("pw"),
      code: sel.dataset.age,
      min_age: sel.value ? Number(sel.value) : null,
    });
    // 認められなかったら表示を元の設定に戻す
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

// ブラウザは操作があるまで音を出せないので、最初のタッチ・キー入力で音を準備する
document.addEventListener("pointerdown", unlockAudio, { capture: true });
document.addEventListener("keydown", unlockAudio, { capture: true });

app.addEventListener("click", (e) => {
  const b = (e.target as Element).closest("button");
  if (!b || b.disabled) return;
  if (b.hasAttribute("data-open-rules")) return rulesDlg().showModal();
  if (b.hasAttribute("data-close-rules")) return rulesDlg().close();
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
  if (b.dataset.gear) {
    const reg = Number(b.dataset.gear);
    const open = snap.registers.find((x) => x.id === reg)?.menu;
    send({ type: "open_menu", register: reg, screen: open ? null : "top" });
    return;
  }
  if (b.dataset.outlet) {
    send({ type: "take_outlet", register: Number(b.dataset.outlet) });
    return;
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
  // 明細の「変更」：行を広げて個数変更・取り消し（登録画面の中で完結。コアには決めたときだけ送る）
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
    // 年齢確認は、押した画面（お客様側だけ受け付ける）をコアに伝える
    if (cmd.type === "confirm_bill_payment") cmd.side = b.closest<HTMLElement>("[data-drop]")?.dataset.drop?.split(":")[1] ?? "customer";
    // 個数変更・取り消しが済んだら、広げた行を閉じる
    if (send(cmd) && ["set_qty", "remove_line", "remove_coupon"].includes(cmd.type)) editing.delete(`${cmd.register}:${cmd.side}`);
  }
  else if (b.dataset.form) form(b);
});

// ---------------------------------------------------------------------------
// ドラッグ＆ドロップ（ポインターイベントで実装。マウスでもタッチでも同じに動く）
// ---------------------------------------------------------------------------

function setupDrag(): void {
  const ghost = document.getElementById("ghost")!;
  // code：レジで読むもの（商品・ポイントカード）、wallet：お金（cash:金額）とかざす決済（pay:方法）
  let drag: { code?: string; wallet?: string; label: string; x: number; y: number; moved: boolean; id: number } | null = null;
  let over: Element | null = null;

  const target = (x: number, y: number): Element | null => document.elementFromPoint(x, y)?.closest("[data-drop], [data-cash-drop]") ?? null;
  const highlight = (el: Element | null) => {
    if (over === el) return;
    over?.classList.remove("drop-over");
    el?.classList.add("drop-over");
    over = el;
  };
  const end = () => {
    drag = null;
    ghost.hidden = true;
    highlight(null);
    document.body.classList.remove("dragging");
  };

  app.addEventListener("pointerdown", (e) => {
    const b = (e.target as Element).closest<HTMLElement>("[data-code], [data-wallet]");
    if (!b || e.button !== 0) return;
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
    }
    ghost.style.transform = `translate(${e.clientX + 8}px, ${e.clientY + 8}px)`;
    highlight(target(e.clientX, e.clientY));
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
    if ((e.target as Element).closest("[data-hold]")) e.preventDefault();
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

boot().catch((e) => {
  app.innerHTML = `<p class="error">読み込みに失敗しました：${esc(e)}</p>`;
});
