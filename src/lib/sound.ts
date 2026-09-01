// Thin wrapper around cuelume so the app plays and binds without reaching
// for the package directly. Cuelume synthesises everything through the Web
// Audio API, so there are no asset files to ship and nothing to preload.
//
// Every call is guarded: audio is a decoration here, and a browser that
// refuses to hand out an AudioContext (or a jsdom test that has none at
// all) must not take a window down with it.
//
// Web Audio starts suspended until a user gesture resumes it, and resume()
// only reliably runs while that gesture is still on the stack. A cue fired
// after an await can miss entirely, as on the recording toggle. primeAudio()
// runs synchronously during the gesture to create the shared context and
// start resume before any async work.
import { bind as cuelumeBind, play as cuelumePlay, setEnabled, setVolume } from "cuelume";
import type { SoundName } from "cuelume";

export type { SoundName };
export { sounds } from "cuelume";

let primed = false;

/// Wakes the shared AudioContext during a user gesture. Safe to call more
/// than once; later calls are no-ops. Call before any await in a handler
/// that will play a cue when the async work finishes.
export function primeAudio(): void {
  if (primed || typeof window === "undefined") return;
  try {
    // Inaudible, but non-zero: cuelume skips a play at volume 0.
    cuelumePlay("tick", { volume: 0.001 });
    primed = true;
  } catch {
    // Leave unprimed so the next gesture can retry.
  }
}

export function play(name: SoundName): void {
  primeAudio();
  try {
    cuelumePlay(name);
  } catch {
    // A cue that will not sound is not worth an error.
  }
}

/// Wires up the declarative `data-cuelume-*` attributes. Called once per
/// window, from the top-level view.
export function bind(): void {
  try {
    cuelumeBind();
  } catch {
    // As above: no sound is an acceptable outcome, a crash is not.
  }

  if (typeof document === "undefined") return;

  // Capture phase, so the context begins waking on pointerdown before
  // click handlers run and before any handler awaits backend work.
  const onGesture = () => primeAudio();
  document.addEventListener("pointerdown", onGesture, { capture: true });
  document.addEventListener("keydown", onGesture, { capture: true });
}

/// Applies the user's preferences to the audio engine. Called on load and
/// again whenever the Settings tab changes either value, so what you hear
/// matches what the checkbox says without waiting for a restart.
export function applySoundSettings(enabled: boolean, volume: number): void {
  try {
    setEnabled(enabled);
    setVolume(volume);
  } catch {
    // Ditto.
  }
}
