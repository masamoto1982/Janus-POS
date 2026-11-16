import { describe, expect, it } from "vitest";
import { arrange, seats, shells, wrapUnit } from "./law";

// W=400 の目安：小ボタン 50×25、大ボタン 100×50、特大 141×71、最小間隔 8.8
const g = 400 / 45.255;
const L = 400;
const small = (n: number) => ({ widths: Array(n).fill(50), height: 25 });

describe("レイアウトの法則", () => {
  it("一つの群は中心へ寄り、中は最小間隔で詰める", () => {
    const [p] = arrange([small(3)], L, g);
    expect(p.to).toBe("center");
    expect(p.items[1].x - (p.items[0].x + 50)).toBeCloseTo(g);
    expect(p.items[0].x + p.width / 2).toBeCloseTo(L / 2);
  });

  it("いちばん目立たせたいものが中央、残りは主役の前が左・後が右（待ち受けの帯：Language｜呼び鈴｜電子マネー）", () => {
    const side = { widths: [141], height: 71, weight: 18 };
    const placed = arrange([side, { widths: [50], height: 50, weight: 31 }, side], L, g);
    expect(placed.map((p) => p.to)).toEqual(["left", "center", "right"]);
    expect(placed[0].items[0].x).toBeCloseTo(g);
    expect(placed[2].items[0].x + 141).toBeCloseTo(L - g);
    expect(placed[1].items[0].x + 25).toBeCloseTo(L / 2);
  });

  it("主役がいなければ脇役が繰り上がって中央に座る（呼び鈴のない帯）", () => {
    const [p] = arrange([{ widths: [141, 141], height: 71, weight: 18 }], L, g);
    expect(p.to).toBe("center");
    expect((p.items[0].x + p.items[1].x + 141) / 2).toBeCloseTo(L / 2);
  });

  it("脇役は向きで縁を選ぶ：戻るは左、進むは右。一つだけでも中央へは行かない", () => {
    const total = { weight: 25 };
    expect(seats([{ weight: 12.5, dir: "back" }, total, { weight: 12.5, dir: "go" }])).toEqual(["left", "center", "right"]);
    expect(seats([{ weight: 12.5, dir: "back" }, total])).toEqual(["left", "center"]);
    expect(seats([total, { weight: 12.5, dir: "go" }])).toEqual(["center", "right"]);
    expect(seats([total, { weight: 12.5, dir: "back" }, { weight: 12.5, dir: "go" }])).toEqual(["center", "left", "right"]);
  });

  it("同格は全員が主役", () => {
    expect(seats([{ weight: 18 }, { weight: 18 }, { weight: 18 }])).toEqual(["center", "center", "center"]);
    expect(seats([{}, {}])).toEqual(["center", "center"]);
  });

  it("入るうちは一行（案内ボタン6つ）", () => {
    expect(wrapUnit(small(6), L - 2 * g, g, Infinity).rows.map((r) => r.length)).toEqual([6]);
  });

  it("溢れたら行の数が最も少なく均等な形に、多い行を上に（殻）", () => {
    expect(shells(7, 2)).toEqual([4, 3]);
    expect(wrapUnit(small(7), L - 2 * g, g, Infinity).rows.map((r) => r.length)).toEqual([4, 3]);
    expect(wrapUnit(small(12), L - 2 * g, g, Infinity).rows.map((r) => r.length)).toEqual([6, 6]);
    expect(wrapUnit(small(13), L - 2 * g, g, Infinity).rows.map((r) => r.length)).toEqual([5, 4, 4]);
  });

  it("折り返した行はそれぞれ中央にそろう", () => {
    const [p] = arrange([small(7)], L, g);
    const row2 = p.items.slice(4);
    const mid = (row2[0].x + row2[2].x + 50) / 2;
    expect(mid).toBeCloseTo(L / 2);
  });

  it("次の行の余地がなければ縮めて一行に収める（最下段の大ボタン4つ）", () => {
    const { rows } = wrapUnit({ widths: Array(4).fill(100), height: 50 }, L - 2 * g, g, 60);
    expect(rows).toHaveLength(1);
    const used = rows[0].reduce((s, w) => s + w, 0) + 3 * g;
    expect(used).toBeCloseTo(L - 2 * g);
  });
});
