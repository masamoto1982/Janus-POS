// 起動時のスプラッシュ画面（user 2026-11-22）。Ajisai のスプラッシュ（initSplashScreen）を流用し、
// ロゴ・名前のあとにシミュレーターの使い方を出す。上部の「使い方 i」に気づかないとデモが成り立たないので、
// 初めて開いたときは必ず目に入る形にした。クリック・タップ・キー入力で閉じ、待たせない。
// 一つのタブで一度だけ（作業中の再読み込みでは出さない）。「使い方 i」を押すといつでも出し直せる。
// wasm の読み込みを待たない（DOM だけで動く）
import logoUrl from "./logo.jpg";
import { esc } from "./view";

const SESSION_KEY = "janus-splash-seen";
const STEPS_REVEAL_DELAY_MS = 600;

/** スプラッシュに出す使い方（シミュレーターまわりの文言なので英訳しない、文末の「。」は付けない） */
export const HOW_TO_STEPS: readonly { head: string; body: string }[] = [
  {
    head: "機器を右クリック（タッチは長押し）",
    body: "レジの画面・入金ボックス・決済端末・スキャナ",
  },
  {
    head: "Item か Wallet から選ぶ",
    body: "Item＝売り場の商品、Wallet＝お金・カード",
  },
  {
    head: "その機器で読んだことになる",
    body: "商品をスキャン・お金を投入・カードをかざす など",
  },
];

function markSeen(): void {
  try {
    sessionStorage.setItem(SESSION_KEY, "1");
  } catch {
    // 保存できなくても次の読み込みでもう一度出るだけ
  }
}

function seen(): boolean {
  try {
    return sessionStorage.getItem(SESSION_KEY) === "1";
  } catch {
    return false;
  }
}

function build(): HTMLElement {
  const el = document.createElement("div");
  el.className = "splash-screen";
  el.setAttribute("role", "dialog");
  el.setAttribute("aria-modal", "true");
  el.setAttribute("aria-label", "Janus POS の使い方");
  el.tabIndex = -1;
  el.innerHTML = `
    <div class="splash-content">
      <img class="splash-logo" src="${logoUrl}" width="96" height="96" alt="" />
      <div class="splash-heading">
        <p class="splash-title">Janus POS</p>
        <span class="build-badge">${esc(String(import.meta.env.VITE_BUILD))}</span>
      </div>
      <p class="splash-lead">コンビニ向けセミセルフ POS レジのシミュレーター</p>
      <section class="splash-howto" aria-label="使い方">
        <h2 class="splash-howto-head">使い方</h2>
        <ol class="splash-steps">
          ${HOW_TO_STEPS.map(
            (s, i) => `<li class="splash-step"><span class="splash-step-no" aria-hidden="true">${i + 1}</span><span class="splash-step-text"><strong>${esc(s.head)}</strong><span>${esc(s.body)}</span></span></li>`,
          ).join("")}
        </ol>
      </section>
      <p class="splash-hint">クリック・タップ・キー入力ではじめる<br />上部の <span class="info-icon" aria-label="使い方">i</span> でいつでも見直せます</p>
    </div>`;
  return el;
}

/** スプラッシュを出す。初回は自動で、二度目以降は「使い方 i」から */
export function openSplash(): void {
  if (document.querySelector(".splash-screen")) return;
  const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  const splash = build();
  document.body.append(splash);
  splash.focus();
  window.setTimeout(() => splash.classList.add("splash-detail-visible"), STEPS_REVEAL_DELAY_MS);

  let dismissed = false;
  const dismiss = (): void => {
    if (dismissed) return;
    dismissed = true;
    markSeen();
    splash.classList.add("splash-dismissing");
    splash.addEventListener("transitionend", (event) => {
      // 使い方の行の出現アニメーションも泡立って届くので、スプラッシュ自身の終わりだけで消す
      if (event.target !== splash) return;
      splash.remove();
    });
    // 動きを減らす設定などで transitionend が来ないとき
    window.setTimeout(() => splash.remove(), 400);
    opener?.focus({ preventScroll: true });
  };
  splash.addEventListener("click", dismiss);
  splash.addEventListener("keydown", (e) => {
    e.preventDefault();
    dismiss();
  });
}

/** 起動時に一度だけ。同じタブで出したことがあれば出さない */
export function initSplash(): void {
  if (seen()) return;
  openSplash();
}
