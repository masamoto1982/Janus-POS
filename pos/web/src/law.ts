// レイアウトの法則（user 2026-11-16）：ボタンの数が変わっても、置き場所を手で決め直さなくてよいように、
// 行の中の部品の位置を次の法則だけで決める。
//
// 1. 同じ種類の部品が続くところは一つの「群」になる。群の中は最小間隔で詰める
// 2. 群（と単独の部品）は、中心か縁のうち近い方へ寄る（出発点は均等に並べた位置）
// 3. 部品同士、部品と縁は最小間隔（--gap）より近づかない
// 4. 群が一行に入らないとき、次の行を置く余地があれば折り返す。行は一つずつ満たし（殻）、
//    行の数が最も少なく、各行がなるべく均等な形にして中央にそろえる。余地がなければ部品を縮めて一行に収める
//
// 位置の計算（arrange）は DOM に触れない純粋関数。DOM への当てはめは applyLaw が行う。

export type Attractor = "left" | "center" | "right";

export interface Unit {
  /** 群の部品の幅（単独の部品は要素一つ） */
  widths: number[];
  /** 部品の高さ。行の高さは群の中でいちばん高いもの（height）、各部品は行の中で上下の中央 */
  heights?: number[];
  height: number;
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
  // 出発点：均等に並べた位置（space-evenly）。そこから中心か縁の近い方へ
  const free = L - shaped.reduce((s, x) => s + x.width, 0);
  const step = free / (shaped.length + 1);
  let x = step;
  const to: Attractor[] = shaped.map((s) => {
    const c = x + s.width / 2;
    x += s.width + step;
    const toCenter = Math.abs(c - L / 2);
    const toEdge = Math.min(c - s.width / 2, L - (c + s.width / 2));
    return toCenter <= toEdge ? "center" : c < L / 2 ? "left" : "right";
  });
  const left: number[] = [];
  const right: number[] = [];
  const mid: number[] = [];
  to.forEach((t, i) => (t === "left" ? left : t === "right" ? right : mid).push(i));
  const at: number[] = [];
  x = gap;
  for (const i of left) {
    at[i] = x;
    x += shaped[i].width + gap;
  }
  const leftEnd = x;
  x = L - gap;
  for (const i of [...right].reverse()) {
    x -= shaped[i].width;
    at[i] = x;
    x -= gap;
  }
  const rightStart = x + gap;
  const midW = rowWidth(mid.map((i) => shaped[i].width), gap);
  let mx = Math.min(Math.max(L / 2 - midW / 2, leftEnd), rightStart - gap - midW);
  for (const i of mid) {
    at[i] = mx;
    mx += shaped[i].width + gap;
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
    for (const k of kids) for (const p of ["left", "top", "width"]) k.style.removeProperty(p);
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
      } else units.push({ els: [k], unit: { widths: [sizes[i].width], heights: [sizes[i].height], height: sizes[i].height } });
    });
    const placed = arrange(units.map((u) => u.unit), L, gap, maxHeight);
    const offset = area.getBoundingClientRect().left - tierLeft;
    area.classList.add("law-on");
    area.style.height = `${Math.max(...placed.map((p) => p.height))}px`;
    units.forEach((u, i) =>
      u.els.forEach((el, j) => {
        const p = placed[i].items[j];
        el.style.left = `${p.x - offset}px`;
        el.style.top = `${p.y}px`;
        el.style.width = `${p.w}px`;
      }),
    );
  }
}
