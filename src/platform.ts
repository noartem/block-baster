import { App } from '@capacitor/app';
import { Capacitor } from '@capacitor/core';
import { Haptics, ImpactStyle } from '@capacitor/haptics';

export const isNative = Capacitor.isNativePlatform();

/**
 * Whether the device can vibrate. Desktop Chrome exposes `navigator.vibrate`
 * without any motor behind it, so on the web only touch-first devices count.
 */
export const canVibrate =
  isNative || (typeof navigator !== 'undefined' && 'vibrate' in navigator && typeof matchMedia === 'function' && matchMedia('(pointer: coarse)').matches);

const LEVELS = ['light', 'medium', 'heavy'] as const;
const WEB_MS = { light: 8, medium: 18, heavy: 35 };

/**
 * Short vibration feedback; uses the native haptics engine inside the app.
 * `intensity` 0..1 scales it: 0.5 is the normal strength, 0 is off.
 */
export function vibrate(strength: (typeof LEVELS)[number], intensity = 0.5): void {
  if (!canVibrate || intensity <= 0) return;
  const scale = intensity * 2;
  if (isNative) {
    // The native engine has three strengths; a stronger setting moves each kick up a level.
    const level = Math.round((LEVELS.indexOf(strength) + 1) * scale) - 1;
    const style = [ImpactStyle.Light, ImpactStyle.Medium, ImpactStyle.Heavy][Math.max(0, Math.min(2, level))];
    void Haptics.impact({ style }).catch(() => {});
  } else {
    try {
      navigator.vibrate(Math.max(4, Math.round(WEB_MS[strength] * scale)));
    } catch {
      // Ignore: some browsers block vibration without user activation.
    }
  }
}

/** Calls the handler on the Android hardware back button. */
export function onBackButton(handler: () => void): void {
  if (isNative) void App.addListener('backButton', handler);
}

export function exitApp(): void {
  if (isNative) void App.exitApp();
}
