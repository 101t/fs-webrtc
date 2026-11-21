// Ring and ringback tones synthesized with Web Audio: no sound files to ship.

export type Tone = "ring" | "ringback";

const TONES = {
  // Incoming call: two short chimes every 3 s.
  ring: { freqs: [523.25, 659.25], level: 0.18, period: 3, bursts: [[0, 0.4], [0.6, 1.0]] },
  // Outgoing, while the other side rings: 2 s on, 4 s off.
  ringback: { freqs: [440, 480], level: 0.06, period: 6, bursts: [[0, 2]] },
} as const;

/** Starts a repeating tone; call the returned function to stop it. */
export function playTone(kind: Tone): () => void {
  const t = TONES[kind];
  const ctx = new AudioContext();
  const gain = ctx.createGain();
  gain.gain.value = 0;
  gain.connect(ctx.destination);
  for (const f of t.freqs) {
    const osc = ctx.createOscillator();
    osc.frequency.value = f;
    osc.connect(gain);
    osc.start();
  }
  let next = ctx.currentTime + 0.05;
  const schedule = () => {
    while (next < ctx.currentTime + 2 * t.period) {
      for (const [on, off] of t.bursts) {
        gain.gain.setTargetAtTime(t.level, next + on, 0.01);
        gain.gain.setTargetAtTime(0, next + off, 0.01);
      }
      next += t.period;
    }
  };
  schedule();
  const timer = setInterval(schedule, 1000);
  return () => {
    clearInterval(timer);
    void ctx.close();
  };
}
