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

export function beepScan(): void {
  if (!ready()) return;
  tone(2400, 0, 0.09, "square", 0.08);
}

/** 電子マネー・クレジットカードの決済音「シャーン！」：高い倍音の鈴を素早く重ね、長めに余韻を残す */
export function chimePay(): void {
  if (!ready()) return;
  const notes = [2093, 2637, 3136, 4186, 5274];
  notes.forEach((f, i) => {
    tone(f, i * 0.018, 0.9 - i * 0.08, "sine", 0.07);
    tone(f * 2.01, i * 0.018, 0.35, "sine", 0.02);
  });
}

/** 照合OK（年確・FFのお渡し）の「ピンポン」：上がる二音で、確かめられたことを耳でも伝える */
export function chimeOk(): void {
  if (!ready()) return;
  tone(1319, 0, 0.18, "sine", 0.09);
  tone(1760, 0.16, 0.32, "sine", 0.09);
}

export function beepPress(): void {
  if (!ready()) return;
  tone(900, 0, 0.04, "triangle", 0.05);
}

export function beepError(): void {
  if (!ready()) return;
  tone(220, 0, 0.16, "square", 0.07);
  tone(220, 0.2, 0.22, "square", 0.07);
}

let ring: { bus: GainNode; timer: number } | null = null;

function ringPattern(bus: GainNode): void {
  tone(1320, 0, 0.18, "triangle", 0.28, bus);
  tone(990, 0.2, 0.25, "triangle", 0.28, bus);
}

export function setBellRinging(on: boolean, volume: number): void {
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
