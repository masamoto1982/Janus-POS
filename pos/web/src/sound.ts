// 実機の音を Web Audio で鳴らす（音源ファイルは使わず、その場で合成する）。
// ブラウザは操作があるまで音を出せないので、最初のタッチで準備する。
// 呼び出しの音（MAT）は聞き逃しを防ぐため、消す手段を設けない。

let ctx: AudioContext | null = null;

export function unlockAudio(): void {
  if (!ctx) {
    const AC = window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!AC) return;
    ctx = new AC();
  }
  if (ctx.state === "suspended") void ctx.resume();
}

function tone(freq: number, start: number, dur: number, type: OscillatorType, gain: number, out?: AudioNode): void {
  if (!ctx) return;
  const t = ctx.currentTime + start;
  const osc = ctx.createOscillator();
  const g = ctx.createGain();
  osc.type = type;
  osc.frequency.value = freq;
  // クリック音が出ないよう、立ち上がりと減衰をつける
  g.gain.setValueAtTime(0, t);
  g.gain.linearRampToValueAtTime(gain, t + 0.005);
  g.gain.exponentialRampToValueAtTime(0.0001, t + dur);
  osc.connect(g).connect(out ?? ctx.destination);
  osc.start(t);
  osc.stop(t + dur + 0.02);
}

function ready(): boolean {
  return ctx !== null && ctx.state === "running";
}

/** 商品を登録したときの「ピッ」 */
export function beepScan(): void {
  if (!ready()) return;
  tone(2400, 0, 0.09, "square", 0.08);
}

/** ボタンを押したときの「コッ」（押した表現は色を変えず、位置のずれとこの音だけ） */
export function beepPress(): void {
  if (!ready()) return;
  tone(900, 0, 0.04, "triangle", 0.05);
}

/** 読めなかった・受け付けられなかったときの「ブッブー」 */
export function beepError(): void {
  if (!ready()) return;
  tone(220, 0, 0.16, "square", 0.07);
  tone(220, 0.2, 0.22, "square", 0.07);
}

// ---------------------------------------------------------------------------
// 呼び出し音
//
// - 呼び出しを順番待ちにしない：キュー（FIFO）に並べると、前の呼び出しのせいで後の
//   呼び出しの知らせが遅れる。実機で問題になっている罠なので、MAT の鳴動はコアが
//   鳴らすと決めた瞬間にそのまま鳴らす。
// - 鳴るのはお客様が呼び出しボタンに触れている間だけ。鳴らし直し（スヌーズ）はない。
// - 音を出すのは MAT だけ。レジの呼び出しはレジ本体のランプ（画面上の表示）で知らせる。
//   音源が一つなので、複数の音が重なって曲のように聞こえることもない。
// - 放置ほど大きく：音量はコアの ring_volume（最も長く待たせている呼び出しの放置時間）。
// ---------------------------------------------------------------------------

/** MAT の鳴動。on の間だけ鳴り続け、off で即座に止む（お客様がボタンから指を離したとき）。
 *  volume は 0〜1。鳴っている途中で音量が変わればそのまま反映する */
let ring: { bus: GainNode; timer: number } | null = null;

function ringPattern(bus: GainNode): void {
  tone(1320, 0, 0.18, "triangle", 0.28, bus);
  tone(990, 0.2, 0.25, "triangle", 0.28, bus);
}

export function setMatRinging(on: boolean, volume: number): void {
  if (!on || !ready() || !ctx) {
    if (ring) {
      window.clearInterval(ring.timer);
      ring.bus.disconnect();
      ring = null;
    }
    return;
  }
  const v = Math.max(0.05, Math.min(1, volume || 0.25));
  if (ring) {
    ring.bus.gain.value = v;
    return;
  }
  const bus = ctx.createGain();
  bus.gain.value = v;
  bus.connect(ctx.destination);
  ringPattern(bus);
  ring = { bus, timer: window.setInterval(() => ringPattern(bus), 500) };
}
