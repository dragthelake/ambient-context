// Development-only diagnostics for the audio path. Installed by sound.ts in
// dev builds; does nothing in production. Everything goes to the console
// with a `[sound]` prefix so it can be filtered.
//
// What it watches, and why:
// - The AudioContext's state changes, with a timestamp. WebKit moves a
//   context to "suspended" or "interrupted" on its own (hidden page, another
//   app taking audio), and cuelume then queues the cue behind resume().
// - How long resume() takes when it is called.
// - Whether the audio clock keeps pace with the wall clock while idle. A
//   stall means the render pipeline or the output device stopped and is
//   waking up; a cue scheduled into that window plays when it ends.
// - For each cue, how long the render thread takes to reach a marker
//   scheduled at currentTime. Milliseconds is healthy; a second is the lag
//   being chased.
// - Page visibility and window focus, which are what WebKit keys its own
//   suspensions to.

import { invoke } from "@tauri-apps/api/core";

let context: AudioContext | null = null;
let installed = false;

const stamp = () =>
  `${new Date().toISOString().slice(11, 23)} t=${(performance.now() / 1000).toFixed(3)}s`;
// Console for the inspector, and the sound_diag command for a file in the
// app data dir, so the trace survives without the inspector open.
const log = (message: string) => {
  const line = `${stamp()} ${message}`;
  console.log(`[sound] ${line}`);
  void invoke("sound_diag", { line }).catch(() => undefined);
};

function watchClock(ctx: AudioContext): void {
  let lastWall = performance.now();
  let lastAudio = ctx.currentTime;
  setInterval(() => {
    const wall = performance.now();
    const audio = ctx.currentTime;
    const wallMs = wall - lastWall;
    const audioMs = (audio - lastAudio) * 1000;
    // Allow a render quantum or two of jitter; flag anything larger.
    if (ctx.state === "running" && wallMs - audioMs > 150) {
      log(`audio clock stalled ${Math.round(wallMs - audioMs)}ms (state ${ctx.state})`);
    }
    lastWall = wall;
    lastAudio = audio;
  }, 250);
}

/// Wraps window.AudioContext so the context cuelume creates privately can
/// still be observed. Must run before the first play.
export function installSoundDiag(): void {
  if (installed || typeof window === "undefined" || !window.AudioContext) return;
  installed = true;
  const Ctor = window.AudioContext;
  window.AudioContext = class extends Ctor {
    constructor(options?: AudioContextOptions) {
      super(options);
      context = this;
      log(`context created, state ${this.state}, sampleRate ${this.sampleRate}`);
      this.addEventListener("statechange", () => {
        log(`state -> ${this.state}`);
      });
      const realResume = this.resume.bind(this);
      this.resume = () => {
        const t0 = performance.now();
        log("resume() called");
        return realResume().then(
          (r) => {
            log(`resume() resolved in ${Math.round(performance.now() - t0)}ms, state ${this.state}`);
            return r;
          },
          (error) => {
            log(`resume() rejected after ${Math.round(performance.now() - t0)}ms: ${String(error)}`);
            throw error;
          },
        );
      };
      watchClock(this);
    }
  };
  document.addEventListener("visibilitychange", () => {
    log(`visibility ${document.visibilityState}`);
  });
  window.addEventListener("focus", () => log("window focus"));
  window.addEventListener("blur", () => log("window blur"));
}

/// Called by sound.play() just before it hands the cue to cuelume.
export function markPlay(name: string): void {
  if (!context) {
    log(`play(${name}) before any context exists`);
    return;
  }
  const ctx = context;
  const t0 = performance.now();
  log(
    `play(${name}) state ${ctx.state}, outputLatency ${Math.round((ctx.outputLatency ?? 0) * 1000)}ms`,
  );
  if (ctx.state !== "running") return;
  // One silent sample at currentTime: onended fires when the render
  // thread has passed it, which is when the cue itself starts.
  const marker = ctx.createBufferSource();
  marker.buffer = ctx.createBuffer(1, 1, ctx.sampleRate);
  marker.connect(ctx.destination);
  marker.onended = () => {
    const ms = Math.round(performance.now() - t0);
    log(`play(${name}) render thread reached it after ${ms}ms${ms > 50 ? "  <-- LATE" : ""}`);
  };
  marker.start(ctx.currentTime);
}
