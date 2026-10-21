// Sound (M7). The core makes every voice's samples from data/sound.json and
// says, each frame, which voices start (how loud, how fast, from which side)
// and how loud each looping voice is now (crates/core/src/sound.rs). This
// file plays what it is told and decides nothing: no level, no timing, no
// choice of sound is made here.
//
// Browsers let a page make sound only after the reader presses something,
// so nothing is set up until the first click or key (`wake`).

export function makeSound(game) {
  let ctx = null;
  let out = null;
  const buffers = [];
  const loops = new Map();
  let smooth = 0;
  let cueValues = 1;
  let loopValues = 1;

  function wake() {
    if (!ctx) {
      const Context = window.AudioContext || window.webkitAudioContext;
      if (!Context) return;
      const [rate, smoothS, threshold, knee, ratio, attack, release, cues, looped, count] = game.sound_settings();
      ctx = new Context();
      smooth = smoothS;
      cueValues = cues;
      loopValues = looped;
      // the limiter keeps a pile of loud voices from clipping
      out = ctx.createDynamicsCompressor();
      out.threshold.value = threshold;
      out.knee.value = knee;
      out.ratio.value = ratio;
      out.attack.value = attack;
      out.release.value = release;
      out.connect(ctx.destination);
      for (let v = 0; v < count; v += 1) {
        const data = game.sound_samples(v);
        const buffer = ctx.createBuffer(1, data.length, rate);
        buffer.copyToChannel(data, 0);
        buffers.push(buffer);
      }
    }
    if (ctx.state === "suspended") ctx.resume().catch(() => {});
  }

  function voice(v, gain, rate, pan, loop) {
    const src = ctx.createBufferSource();
    src.buffer = buffers[v];
    src.loop = loop;
    src.playbackRate.value = rate;
    const level = ctx.createGain();
    level.gain.value = gain;
    src.connect(level);
    let side = null;
    if (ctx.createStereoPanner) {
      side = ctx.createStereoPanner();
      side.pan.value = pan;
      level.connect(side);
      side.connect(out);
    } else {
      level.connect(out);
    }
    return { src, level, side };
  }

  // Each frame, after the core has advanced: start this frame's voices and
  // set every loop to the level the core gives it.
  function frame() {
    if (!ctx) return;
    const cues = game.sounds();
    for (let i = 0; i < cues.length; i += cueValues) {
      const [v, gain, rate, pan] = cues.subarray(i, i + cueValues);
      voice(v, gain, rate, pan, false).src.start();
    }
    const now = ctx.currentTime;
    const levels = game.sound_loops();
    for (let i = 0; i < levels.length; i += loopValues) {
      const [v, gain, rate, pan] = levels.subarray(i, i + loopValues);
      let l = loops.get(v);
      if (!l) {
        if (gain <= 0) continue;
        l = voice(v, 0, rate, pan, true);
        l.src.start();
        loops.set(v, l);
      }
      l.level.gain.setTargetAtTime(gain, now, smooth);
      l.src.playbackRate.setTargetAtTime(rate, now, smooth);
      if (l.side) l.side.pan.setTargetAtTime(pan, now, smooth);
    }
  }

  return { wake, frame, started: () => ctx !== null };
}
