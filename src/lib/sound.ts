// Thin wrapper around cuelume so the app plays and binds without reaching
// for the package directly. Cuelume synthesises everything through the Web
// Audio API, so there are no asset files to ship and nothing to preload.
//
// Every call is guarded: audio is a decoration here, and a browser that
// refuses to hand out an AudioContext (or a jsdom test that has none at
// all) must not take a window down with it.
import { bind as cuelumeBind, play as cuelumePlay, setEnabled, setVolume } from "cuelume";
import type { SoundName } from "cuelume";

export type { SoundName };
export { sounds } from "cuelume";

export function play(name: SoundName): void {
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
