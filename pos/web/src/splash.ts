// 起動時のスプラッシュ画面（user 2026-11-22）。Ajisai のスプラッシュ（initSplashScreen）を流用し、
// ロゴ・名前のあとにシミュレーターの使い方を出す。ヘッダーのアイコンに気づかないとデモが成り立たないので、
// 初めて開いたときは必ず目に入る形にした。クリック・タップ・キー入力で閉じ、待たせない。
// 案内はマウスかタッチかを見分けて、その端末の操作だけを書く（user 2026-11-22）。
// 一つのタブで一度だけ（作業中の再読み込みでは出さない）。ヘッダーの「使用方法」を押すといつでも出し直せる。
// wasm の読み込みを待たない（DOM だけで動く）
import logoUrl from "./logo.jpg";
import { MOBILE } from "./law";
import { esc } from "./view";

const SESSION_KEY = "janus-splash-seen";
const STEPS_REVEAL_DELAY_MS = 600;

/** タッチ操作の端末か（指が主な入力）。案内の言い回しをその端末の操作だけに絞る（user 2026-11-22） */
export function isTouchDevice(): boolean {
  return typeof matchMedia === "function" && matchMedia("(pointer: coarse)").matches;
}

/** モバイルの配置（幅 760px 以下）か。レジカウンター側と売り場側を横スワイプで切り替える配置 */
export function isMobileLayout(): boolean {
  return typeof matchMedia === "function" && matchMedia(MOBILE).matches;
}

/** スプラッシュに出す使い方（user 2026-11-22 の二手順、シミュレーターまわりの文言なので英訳しない、文末の「。」は付けない）。
    マウスの端末には右クリック、タッチの端末には長押しだけを書く。
    モバイルの配置では三つ目に面の切り替えを足す（user 2026-11-22：二つの画面が同時に見えないので） */
export function howToSteps(touch: boolean, mobile = false): readonly string[] {
  const press = touch ? "長押し" : "右クリック";
  return [
    `レジ画面、スキャナを${press}で商品登録またはポイント読み取り`,
    `決済手段に応じ、入金、スキャナ、決済端末のいずれかを${press}`,
    ...(mobile ? ["横スワイプで二つの画面を行き来出来ます"] : []),
  ];
}

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
  const touch = isTouchDevice();
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
          ${howToSteps(touch, isMobileLayout()).map(
            (s, i) => `<li class="splash-step"><span class="splash-step-no" aria-hidden="true">${i + 1}</span><span class="splash-step-text"><strong>${esc(s)}</strong></span></li>`,
          ).join("")}
        </ol>
      </section>
      <p class="splash-hint">${touch ? "タップ" : "クリック"}でデモンストレーションを開始します</p>
    </div>`;
  return el;
}

/** スプラッシュを出す。初回は自動で、二度目以降はヘッダーの「使用方法」から */
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
