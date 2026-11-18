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

/** 向き：バック・取引中止は戻る、積極とスキップは進む */
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
  const tierOf = (b: HTMLElement) => (parts(b) || [...(b.parentElement?.children ?? [])].some((x) => x !== b && x instanceof HTMLElement && x.matches("button") && parts(x)) ? 1 : 0);
  // 1. 幅：文字＋左右の余白 B÷4＋枠（同じ並びでもボタンごと、そろえない）
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
  // 2. 幅を変えられないボタン（RIGID）も文字は決めた段のまま。表示エリアに合わせて文字を下げる仕組みは廃止（user 2026-11-18）
  for (const b of buttons) if (b.matches(RIGID)) apply(b, tierOf(b));
}

