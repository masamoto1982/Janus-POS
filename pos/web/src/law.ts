// レイアウトの法則（user 2026-11-16）：ボタンの数が変わっても、置き場所を手で決め直さなくてよいように、
// 行の中の部品の位置を次の法則だけで決める。
//
// 目立たせたい順に、良い席から座る。良い席とは中央
//
// 1. 目立たせたさは部品の見た目にすでに書いてある：部品の中でいちばん大きい文字（絵だけの部品は絵の高さ）
// 2. いちばん目立たせたいものが主役で、中央に座る。同格が複数なら全員が主役。中央は空けない
// 3. 残りは脇役。向きのある脇役は縁に座る：戻る部品は左、進む部品（積極・スキップ）は右。
//    向きのない脇役は縁へ行く理由がないので、主役の両脇に最小間隔で寄り添う（主役より前に書いたものが左、後に書いたものが右）。
//    主役の位置は脇役によって動かない（待ち受けの帯：Language｜呼び鈴｜電子マネー、呼び鈴は中央のまま）
// 4. 同じ種類の部品が続くところは一つの群。群の中は最小間隔で詰める。部品同士、部品と縁は最小間隔（--gap）より近づかない
// 5. 群が一行に入らないとき、次の行を置く余地があれば折り返す。行は中央から満たし（殻）、
//    行の数が最も少なく、各行がなるべく均等な形にして中央にそろえる。余地がなければ部品を縮めて一行に収める
//
// 位置の計算（arrange）は DOM に触れない純粋関数。DOM への当てはめは applyLaw が行う。

/** left・right＝縁、center＝中央（主役）、beside-left・beside-right＝主役の左右に寄り添う */
export type Attractor = "left" | "center" | "right" | "beside-left" | "beside-right";

export interface Unit {
  /** 群の部品の幅（単独の部品は要素一つ） */
  widths: number[];
  /** 部品の高さ。行の高さは群の中でいちばん高いもの（height）、各部品は行の中で上下の中央 */
  heights?: number[];
  height: number;
  /** 目立たせたさ（いちばん大きい文字の大きさ）。省けば全員が同格＝主役 */
  weight?: number;
  /** 脇役になったときの向き：戻る＝左の縁、進む＝右の縁 */
  dir?: "back" | "go";
}

/** 同格とみなす幅：文字の四段は √2 倍ずつ離れているので、5% 以内なら同じ段 */
const SAME_TIER = 0.95;

/** 席を決める：いちばん目立たせたいものが中央、向きのある脇役は縁、向きのない脇役は主役の両脇（主役の前が左・後が右） */
export function seats(units: Pick<Unit, "weight" | "dir">[]): Attractor[] {
  const top = Math.max(...units.map((u) => u.weight ?? 0));
  const lead = units.map((u) => (u.weight ?? 0) >= top * SAME_TIER);
  const first = lead.indexOf(true);
  return units.map((u, i) => (lead[i] ? "center" : u.dir === "back" ? "left" : u.dir === "go" ? "right" : i < first ? "beside-left" : "beside-right"));
}

export interface Placed {
  to: Attractor;
  /** 部品ごとの位置・幅（エリアの左上が原点） */
  items: { x: number; y: number; w: number }[];
  /** 群の外形 */
  width: number;
  height: number;
}

/** 殻の折り返し：n 個を k 行に、なるべく均等に（多い行を上に） */
export function shells(n: number, k: number): number[] {
  const base = Math.floor(n / k);
  const extra = n % k;
  return Array.from({ length: k }, (_, i) => base + (i < extra ? 1 : 0));
}

const rowWidth = (ws: number[], gap: number) => ws.reduce((s, w) => s + w, 0) + gap * Math.max(0, ws.length - 1);

/** 群を行に分ける。入りきらず、次の行の余地もなければ縮めて一行に */
export function wrapUnit(u: Unit, room: number, gap: number, maxHeight: number): { rows: number[][]; height: number } {
  const n = u.widths.length;
  for (let k = 1; k <= n; k++) {
    const height = k * u.height + (k - 1) * gap;
    if (k > 1 && height > maxHeight) break;
    const sizes = shells(n, k);
    const rows: number[][] = [];
    let i = 0;
    for (const s of sizes) {
      rows.push(u.widths.slice(i, i + s));
      i += s;
    }
    if (rows.every((r) => rowWidth(r, gap) <= room + 0.5)) return { rows, height };
  }
  // 縮む：一行のまま、すき間は最小間隔を保って部品の幅を同じ比率で縮める
  const scale = Math.max(0, room - gap * (n - 1)) / u.widths.reduce((s, w) => s + w, 0);
  return { rows: [u.widths.map((w) => w * scale)], height: u.height };
}

/**
 * 一行のエリア（幅 L）に単位を並べる。高さ maxHeight まで折り返してよい。
 * 返り値は単位ごとの位置。エリアの高さは返り値の height の最大
 */
export function arrange(units: Unit[], L: number, gap: number, maxHeight = Infinity): Placed[] {
  const room = L - 2 * gap;
  const shaped = units.map((u) => {
    const { rows, height } = wrapUnit(u, room, gap, maxHeight);
    return { u, rows, height, width: Math.max(...rows.map((r) => rowWidth(r, gap))) };
  });
  const to = seats(units);
  const pick = (t: Attractor) => to.flatMap((x, i) => (x === t ? [i] : []));
  const at: number[] = [];
  // 縁：左は左から、右は右から詰める
  let x = gap;
  for (const i of pick("left")) {
    at[i] = x;
    x += shaped[i].width + gap;
  }
  const leftEnd = x;
  x = L - gap;
  for (const i of pick("right").reverse()) {
    x -= shaped[i].width;
    at[i] = x;
    x -= gap;
  }
  const rightEnd = x;
  // 中央：主役の群の中心を段の中心に。寄り添う脇役はその外側に最小間隔で並ぶ
  const mid = pick("center");
  const before = pick("beside-left");
  const after = pick("beside-right");
  const midW = rowWidth(mid.map((i) => shaped[i].width), gap);
  const beforeW = before.reduce((s, i) => s + shaped[i].width + gap, 0);
  const afterW = after.reduce((s, i) => s + shaped[i].width + gap, 0);
  let mx = L / 2 - midW / 2;
  // 縁の部品とぶつかるときだけ、まとまりごとずらす
  mx = Math.min(Math.max(mx, leftEnd + beforeW), rightEnd - afterW - midW);
  for (const i of mid) {
    at[i] = mx;
    mx += shaped[i].width + gap;
  }
  x = at[mid[0]];
  for (const i of [...before].reverse()) {
    x -= gap + shaped[i].width;
    at[i] = x;
  }
  x = mx - gap;
  for (const i of after) {
    x += gap;
    at[i] = x;
    x += shaped[i].width;
  }
  const H = Math.max(...shaped.map((s) => s.height));
  return shaped.map((s, i) => {
    const items: Placed["items"] = [];
    const top = (H - s.height) / 2;
    let k = 0;
    s.rows.forEach((r, ri) => {
      // 行は群の中で中央にそろえる
      let rx = at[i] + (s.width - rowWidth(r, gap)) / 2;
      for (const w of r) {
        const h = s.u.heights?.[k++] ?? s.u.height;
        items.push({ x: rx, y: top + ri * (s.u.height + gap) + (s.u.height - h) / 2, w });
        rx += w + gap;
      }
    });
    return { to: to[i], items, width: s.width, height: s.height };
  });
}

/** 目立たせたさ：部品の中でいちばん大きい文字。絵だけの部品（呼び鈴）は絵の高さ */
function weightOf(el: HTMLElement): number {
  let w = 0;
  const walk = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
  for (let n = walk.nextNode(); n; n = walk.nextNode()) if (n.nodeValue?.trim() && n.parentElement) w = Math.max(w, parseFloat(getComputedStyle(n.parentElement).fontSize) || 0);
  for (const pic of el.querySelectorAll("svg, img")) w = Math.max(w, pic.getBoundingClientRect().height);
  return w;
}

/** 向き：バック・取消しは戻る、積極とスキップは進む */
function dirOf(el: HTMLElement): Unit["dir"] {
  const has = (sel: string) => el.matches(sel) || !!el.querySelector(sel);
  return has(".back-btn, .void-btn") ? "back" : has(".primary, .skip") ? "go" : undefined;
}

/** 部品の種類：同じ種類が続くところが群になる */
const kindOf = (el: Element) => (el as HTMLElement).dataset.kind ?? el.classList[0] ?? el.tagName;

/**
 * root の中の [data-law] を法則で並べ直す。各 [data-law] の子要素が部品。
 * エリアの幅は、いちばん近い段（.screen-main／.screen-items）の内幅。縁からの最小間隔もその段の縁から測る
 */
export function applyLaw(root: ParentNode): void {
  for (const area of root.querySelectorAll<HTMLElement>("[data-law]")) {
    const kids = [...area.children] as HTMLElement[];
    // 測り直すために、前回の当てはめを外す
    area.style.removeProperty("height");
    area.classList.remove("law-on");
    // 幅を中身で決めたボタン（強い角丸）は、その幅に戻す
    for (const k of kids) for (const p of ["left", "top", "width", "height"]) k.style.removeProperty(p);
    for (const k of kids) if (k.dataset.fitW) k.style.width = k.dataset.fitW;
    const shown = kids.filter((k) => k.getClientRects().length > 0);
    if (!shown.length) continue;
    const tier = area.closest<HTMLElement>(".screen-main, .screen-items") ?? area.parentElement!;
    const gap = parseFloat(getComputedStyle(area).getPropertyValue("--gap")) || 8;
    const tr = tier.getBoundingClientRect();
    const tierLeft = tr.left + tier.clientLeft;
    const L = tier.clientWidth;
    // 縦の余地：段の高さから、この行以外の中身の高さを引いたもの
    const others = [...tier.children].filter((c) => c !== area && !c.contains(area)).reduce((s, c) => s + c.getBoundingClientRect().height, 0);
    const inner = area.parentElement && area.parentElement !== tier ? [...area.parentElement.children].filter((c) => c !== area).reduce((s, c) => s + c.getBoundingClientRect().height, 0) : 0;
    const maxHeight = Math.max(0, tier.clientHeight - others - inner - 4 * gap);
    const sizes = shown.map((k) => k.getBoundingClientRect());
    const units: { els: HTMLElement[]; unit: Unit }[] = [];
    shown.forEach((k, i) => {
      const last = units[units.length - 1];
      if (last && kindOf(last.els[0]) === kindOf(k)) {
        last.els.push(k);
        last.unit.widths.push(sizes[i].width);
        last.unit.heights!.push(sizes[i].height);
        last.unit.height = Math.max(last.unit.height, sizes[i].height);
        last.unit.weight = Math.max(last.unit.weight ?? 0, weightOf(k));
      } else units.push({ els: [k], unit: { widths: [sizes[i].width], heights: [sizes[i].height], height: sizes[i].height, weight: weightOf(k), dir: dirOf(k) } });
    });
    const placed = arrange(units.map((u) => u.unit), L, gap, maxHeight);
    const offset = area.getBoundingClientRect().left - tierLeft;
    // 部品は行の内側（padding の内側）に置く
    const padTop = parseFloat(getComputedStyle(area).paddingTop) || 0;
    area.classList.add("law-on");
    area.style.height = `${Math.max(...placed.map((p) => p.height))}px`;
    units.forEach((u, i) =>
      u.els.forEach((el, j) => {
        const p = placed[i].items[j];
        el.style.left = `${p.x - offset}px`;
        el.style.top = `${p.y + padTop}px`;
        // 入りきらなくても部品と文字は縮めない（user 2026-11-18：表示エリアに合わせて文字を可変にする仕組みは廃止）
        el.style.width = `${Math.max(p.w, u.unit.widths[j])}px`;
      }),
    );
  }
}

/**
 * ボタンの文字の大きさは、親要素であるボタン自身を基準に四段で自動的に決める（user 2026-11-17）。
 * 基準はボタンの短い辺 B（横長のボタンでは高さ、縦長の ▲▼ では幅）。
 *   特大 B÷2・大 B÷2√2・小 B÷4・極小 B÷4√2（√2 ずつ）
 * 一行の文字は特大、二行（主役＋添え字）は大＋極小。入らなくても段は下げない（user 2026-11-18：表示エリアに合わせて文字を可変にする仕組みは廃止）。
 * 一つのボタンの中で同じ大きさの文字が二行になることはないので、一行の文字は折り返さない。
 * 入力欄（数字を打つ欄）はボタンではないので対象外。ボタン以外の文字は、表示エリア（レジ画面の内幅 W）を基準にした四段
 */
const STEPS = [2, 2 * Math.SQRT2, 4, 4 * Math.SQRT2];
export const TEXT_STEP_NAMES = ["特大", "大", "小", "極小"] as const;

/**
 * ボタンの文字は固定し、ボタンの幅は中身で決める（user 2026-11-17）。
 * 文字はボタンの高さ B から決まる：一行は特大 B÷2、二行（主役＋添え字）は大＋極小。
 * 幅は愚直に「文字＋内側の余白＋枠」：左右の余白は上下の余白と同じ B÷4（一行の文字の上下に残る余白と同じ）。
 * 白銀比の倍数で幅を決める方式（2 倍・3 倍…と入るまで広げる）は退役（user 2026-11-17：失敗だった）。
 * 枠の外側の余白は、並びの最小間隔 --gap（配置の法則）。
 * 並ぶ数が決まっていて幅を変えられないボタン（FF の単品・セルフの最下段・テンキー・明細の − ＋・FF のタブ）は
 * 段の幅のまま。文字も決めた段のまま（入らないときに下げる例外は 2026-11-18 に廃止）
 */
/**
 * 一文字差の幅そろえ（user 2026-11-18）：「一文字と二文字」「二文字と三文字」のように文字数が一文字しか違わないボタンは
 * 同じ幅にする。二文字以上違えば（一文字と三文字など）幅は違ってよい。
 * 文字通りには推移律で全部が同じ幅になってしまうので、範囲を「同じ並び（行・群）の同じ大きさのボタン」に限り、
 * その中で文字数を小さい順に並べ、隣との差が一文字以内でつながるものを一組として、組の中でいちばん広い幅にそろえる。
 * 一・二・三文字が同じ並びにあれば三つとも同じ幅、一文字と三文字だけなら別々の幅。
 * 文字数は全角換算（半角は半文字）で、0.5 刻みに丸める
 */
export function alignByLength(items: { chars: number; width: number }[]): number[] {
  const order = items.map((_, i) => i).sort((a, b) => items[a].chars - items[b].chars);
  const out = items.map((x) => x.width);
  let start = 0;
  for (let k = 1; k <= order.length; k++) {
    if (k < order.length && items[order[k]].chars - items[order[k - 1]].chars <= 1 + 1e-6) continue;
    const run = order.slice(start, k);
    const w = Math.max(...run.map((i) => items[i].width));
    for (const i of run) out[i] = w;
    start = k;
  }
  return out;
}

/** ボタンの文字数（全角換算、0.5 刻み）。二行のボタンは主役の行で数える */
function charsOf(b: HTMLElement): number {
  const main = b.querySelector<HTMLElement>(":scope > b") ?? b;
  const r = document.createRange();
  r.selectNodeContents(main);
  const font = parseFloat(getComputedStyle(main).fontSize) || 1;
  return Math.round((r.getBoundingClientRect().width / font) * 2) / 2;
}

/** 同じ並びの同じ大きさのボタンの組：並び＝法則の行（[data-law]）か合計の帯、なければ親要素。大きさ＝高さと文字の段 */
function rowsOf(buttons: HTMLElement[]): HTMLElement[][] {
  const groups = new Map<string, HTMLElement[]>();
  const rows = new Map<Element, number>();
  for (const b of buttons) {
    const row = b.closest("[data-law], .reg-top") ?? b.parentElement;
    if (!row) continue;
    if (!rows.has(row)) rows.set(row, rows.size);
    const key = `${rows.get(row)}:${Math.round(b.offsetHeight)}:${b.dataset.text}`;
    groups.set(key, [...(groups.get(key) ?? []), b]);
  }
  return [...groups.values()].filter((g) => g.length > 1);
}

const RIGID = ".ff-item, .pad-key, .qty-step button, button.tab, .ff-arrow, .bell-btn";

export function fitText(root: ParentNode): void {
  const buttons = [...root.querySelectorAll<HTMLElement>(".screen button, .screen .method-list .seg")].filter(
    (b) => !b.matches(".pad-field, .birth-field, .ffv-input") && b.getClientRects().length > 0,
  );
  const parts = (b: HTMLElement) => {
    const main = b.querySelector<HTMLElement>(":scope > b");
    const sub = b.querySelector<HTMLElement>(":scope > small");
    return main && sub && sub.getClientRects().length ? { main, sub } : null;
  };
  // ボタンの内側の余白は四辺とも B÷8、主役と添え字の間は B÷16（user 2026-11-17：文字が枠に貼り付いていた）
  const pad = (b: HTMLElement) => Math.min(b.offsetWidth, b.offsetHeight) / 8;
  const apply = (b: HTMLElement, k: number) => {
    const base = Math.min(b.offsetWidth, b.offsetHeight);
    const two = parts(b);
    b.dataset.text = TEXT_STEP_NAMES[k];
    b.style.fontSize = `${base / STEPS[k]}px`;
    b.style.lineHeight = "1";
    b.style.whiteSpace = "nowrap";
    // 余白 B÷8 はボタンの外形から測る（枠線はその中に含める）
    b.style.padding = `${Math.max(0, pad(b) - (parseFloat(getComputedStyle(b).borderTopWidth) || 0))}px`;
    for (const el of b.querySelectorAll<HTMLElement>("*")) el.style.fontSize = "inherit";
    if (two) {
      two.main.style.fontSize = `${base / STEPS[k]}px`;
      two.sub.style.fontSize = `${base / STEPS[k + 2]}px`;
      two.sub.style.marginTop = `${base / 16}px`;
    }
  };
  /** 決めた文字の段：一行は特大、二行は大＋極小 */
  // 二行のボタンと同じ並びにある一行のボタン（セルフ最下段の「その他」など）は、隣の主役と同じ大にそろえる
  // テンキーの下段（バック・←・決定）は数字より一段小さい大（user 2026-11-19：「バック」が幅の決まったキーからはみ出していた）
  const tierOf = (b: HTMLElement) => (b.matches(".pad-fn") || parts(b) || [...(b.parentElement?.children ?? [])].some((x) => x !== b && x instanceof HTMLElement && x.matches("button") && parts(x)) ? 1 : 0);
  // 1. 幅：文字＋左右の余白 B÷4＋枠（まずボタンごと）
  const flex = buttons.filter((b) => !b.matches(RIGID));
  for (const b of flex) {
    b.style.removeProperty("width");
    delete b.dataset.fitW;
    apply(b, tierOf(b));
    const r = document.createRange();
    r.selectNodeContents(b);
    const text = r.getBoundingClientRect().width;
    const border = parseFloat(getComputedStyle(b).borderLeftWidth) || 0;
    const w = Math.ceil(text + b.offsetHeight / 2 + 2 * border);
    b.style.width = b.dataset.fitW = `${w}px`;
    b.style.paddingLeft = b.style.paddingRight = "0";
  }
  // 2. 一文字差の幅そろえ（user 2026-11-18）：同じ並びの同じ大きさのボタンで、文字数の差が一文字以内なら長い方の幅にそろえる
  for (const group of rowsOf(flex)) {
    const ws = alignByLength(group.map((b) => ({ chars: charsOf(b), width: b.offsetWidth })));
    group.forEach((b, i) => (b.style.width = b.dataset.fitW = `${ws[i]}px`));
  }
  // 3. 幅を変えられないボタン（RIGID）も文字は決めた段のまま。表示エリアに合わせて文字を下げる仕組みは廃止（user 2026-11-18）
  for (const b of buttons) if (b.matches(RIGID)) apply(b, tierOf(b));
  // 4. モバイルモードと、画面が W の下限より狭いとき（レジを 5 台並べたとき）だけは逆に、ボタンの大きさから文字を決める（user 2026-11-19：表示エリアの制約に応じた便宜的な措置）。
  //    画面の幅で縮んだボタン（ポイントの使い方・テンキーの「バック」など）に文字が収まらなければ、収まるまで文字を縮める。
  //    余白は上の手順と同じ（幅を変えられるボタンは左右 B÷4、変えられないボタンは B÷8）
  const mobile = typeof matchMedia === "function" && matchMedia(MOBILE).matches;
  for (const b of buttons) if (mobile || narrowScreen(b)) shrinkToFit(b, b.matches(RIGID));
}

/** 画面の幅が W の下限（384px＋枠）より狭い。styles.css の @container wrap と同じ境目 */
const NARROW_WRAP_PX = 387;
function narrowScreen(b: HTMLElement): boolean {
  const wrap = b.closest<HTMLElement>(".screen-wrap");
  return !!wrap && wrap.clientWidth > 0 && wrap.clientWidth < NARROW_WRAP_PX;
}

/** モバイルモード（幅 760px 以下）。styles.css の @media と同じ境目 */
export const MOBILE = "(max-width: 760px)";

/** 文字の幅が収まる大きさの比（1 以上なら縮めない） */
export function fitRatio(text: { width: number; height: number }, room: { width: number; height: number }): number {
  if (text.width <= 0 || text.height <= 0) return 1;
  return Math.min(1, room.width / text.width, room.height / text.height);
}

function shrinkToFit(b: HTMLElement, rigid: boolean): void {
  const B = Math.min(b.offsetWidth, b.offsetHeight);
  const r = document.createRange();
  r.selectNodeContents(b);
  const t = r.getBoundingClientRect();
  const k = fitRatio(t, { width: b.clientWidth - (rigid ? B / 4 : B / 2), height: b.clientHeight - B / 8 });
  if (k >= 1) return;
  const scale = (el: HTMLElement) => (el.style.fontSize = `${(parseFloat(getComputedStyle(el).fontSize) || 0) * k}px`);
  const main = b.querySelector<HTMLElement>(":scope > b");
  const sub = b.querySelector<HTMLElement>(":scope > small");
  if (main && sub && sub.getClientRects().length) [main, sub].forEach(scale);
  else scale(b);
}


/**
 * 入力フォームとセレクタの大きさ（user 2026-11-20）：角の丸みの弱いボタンの決まりから角の丸みを除いたものがフォームの仕組み。
 * ボタン・フォーム・セレクタの高さと幅を同じ理屈で決めると、違う部品が並んでも取り回しが楽になる。
 *   高さ B＝ボタンと同じ四段（特大 W/8・大 W/11.3・小 W/16・極小 W/22.6）から CSS が選ぶ
 *   文字＝B÷2（一行のボタンの特大と同じ）。入力例は小テキスト（打ち込む文字より大きくはしない）
 *   幅＝文字＋左右の余白 B÷4＋枠。文字はいちばん長く打ち込んだときの見本（data-fit）か入力例の長い方。
 *     セレクタはいちばん長い選択肢＋開く印（B÷2）
 *   一文字差の幅そろえも同じ：同じ並びの同じ高さのフォーム・セレクタで、文字数が一文字しか違わなければ広い方の幅に
 * レジ画面の外（シミュレーターまわり・販売規則の設定）も同じ仕組み。B はそこで決めた段の高さ
 */
const FORMS = "button.pad-field, select, input:not([type=checkbox], [type=radio], [type=range])";
let ruler: CanvasRenderingContext2D | null | undefined;
function textWidth(text: string, el: HTMLElement, size: number, weight?: string): number {
  if (ruler === undefined) ruler = typeof document !== "undefined" ? document.createElement("canvas").getContext("2d") : null;
  if (!ruler || !text) return 0;
  const cs = getComputedStyle(el);
  ruler.font = `${weight ?? cs.fontWeight} ${size}px ${cs.fontFamily}`;
  return ruler.measureText(text).width;
}

/** 和文の字面の中心が、行の箱の中心よりどれだけ上にあるか（px） */
function inkLift(el: HTMLElement, size: number): number {
  if (ruler === undefined) textWidth("", el, size);
  if (!ruler) return 0;
  const cs = getComputedStyle(el);
  ruler.font = `${cs.fontWeight} ${size}px ${cs.fontFamily}`;
  const m = ruler.measureText("例");
  if (!m.fontBoundingBoxAscent) return 0;
  return ((m.actualBoundingBoxAscent - m.actualBoundingBoxDescent) - (m.fontBoundingBoxAscent - m.fontBoundingBoxDescent)) / 2;
}

/** その場所の小テキストの大きさ（レジ画面は W/32、外は :root の値） */
function smallText(el: HTMLElement): number {
  const probe = document.createElement("span");
  probe.style.cssText = "position:absolute;visibility:hidden;font-size:var(--text-sm)";
  el.parentElement!.appendChild(probe);
  const px = parseFloat(getComputedStyle(probe).fontSize) || 12;
  probe.remove();
  return px;
}

export function fitForms(root: ParentNode): void {
  const forms = [...root.querySelectorAll<HTMLElement>(FORMS)].filter((f) => !f.closest(".qty-pop") && f.getClientRects().length > 0);
  const chars = new Map<HTMLElement, number>();
  for (const f of forms) {
    f.style.removeProperty("width");
    f.style.removeProperty("height");
    f.style.removeProperty("padding-top");
    f.style.removeProperty("padding-bottom");
    const B = f.offsetHeight;
    if (!B) continue;
    const size = B / 2;
    f.style.fontSize = `${size}px`;
    f.style.setProperty("--form-fs", `${size}px`);
    // 入力欄（input）は行の高さを 1 にすると和文の字面が上にずれる（user 2026-11-20）。ボタン形の欄・セレクタと同じく字面の中心で上下中央に
    f.style.lineHeight = f instanceof HTMLInputElement ? "normal" : "1";
    const border = parseFloat(getComputedStyle(f).borderLeftWidth) || 0;
    const isSelect = f instanceof HTMLSelectElement;
    const samples = isSelect ? [...f.options].map((o) => o.text) : [f.dataset.fit ?? (f as HTMLInputElement).value ?? f.textContent ?? ""];
    const main = Math.max(0, ...samples.map((t) => textWidth(t, f, size)));
    // 入力例は小テキスト（user 2026-11-18）。ただし打ち込む文字より大きくはしない（CSS の min(1em, --text-sm) と同じ）
    const span = f.querySelector<HTMLElement>(".placeholder");
    const hintText = span?.textContent ?? (f as HTMLInputElement).placeholder ?? "";
    const hint = span ? span.getBoundingClientRect().width : hintText ? textWidth(hintText, f, Math.min(size, smallText(f)), "400") : 0;
    const mark = isSelect ? B / 2 : 0;
    // 上下の中央は字面の中央で合わせる（user 2026-11-20：区分名の入力例が上にずれていた）。行の箱は欧文の書体で決まり、
    // 和文の字面はその中央より上に載るので、ずれた分だけ上の余白で押し下げる（見本は和文の「例」）
    const lift = inkLift(f, size);
    if (isSelect) {
      // セレクタは固定の高さの中で上の余白を無視して中央に置くので、高さを余白で組み立てる
      const room = B - 2 * border - size;
      f.style.height = "auto";
      f.style.paddingTop = `${Math.max(0, room / 2 + lift)}px`;
      f.style.paddingBottom = `${Math.max(0, room / 2 - lift)}px`;
    } else {
      f.style.paddingTop = `${Math.max(0, 2 * lift)}px`;
      f.style.paddingBottom = "0";
    }
    f.style.paddingLeft = `${Math.max(0, B / 4 - border)}px`;
    f.style.paddingRight = `${Math.max(0, B / 4 + mark - border)}px`;
    f.style.width = `${Math.ceil(Math.max(main, hint) + B / 2 + mark + 2 * border)}px`;
    // 文字数は一文字ずつ数える（数字も一文字：「二文字のフォームと三文字のフォーム」）
    chars.set(f, Math.max(...samples.map((t) => [...t].length)));
  }
  const groups = new Map<string, HTMLElement[]>();
  const rows = new Map<Element, number>();
  for (const f of chars.keys()) {
    const row = f.closest("[data-law], .field-row, tr, .menu-form, .parcel-fields") ?? f.parentElement;
    if (!row) continue;
    if (!rows.has(row)) rows.set(row, rows.size);
    const key = `${rows.get(row)}:${Math.round(f.offsetHeight)}`;
    groups.set(key, [...(groups.get(key) ?? []), f]);
  }
  for (const g of groups.values()) {
    if (g.length < 2) continue;
    const ws = alignByLength(g.map((f) => ({ chars: chars.get(f)!, width: f.offsetWidth })));
    g.forEach((f, i) => (f.style.width = `${ws[i]}px`));
  }
}
