let shared: AudioContext | null = null;

/** The one AudioContext shared by sound effects and music, created on first use. */
export function audioContext(): AudioContext | null {
  if (!shared) {
    const Ctor = window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!Ctor) return null;
    shared = new Ctor();
  }
  return shared;
}
