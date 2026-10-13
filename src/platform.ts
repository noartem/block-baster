import { App } from '@capacitor/app';
import { Capacitor } from '@capacitor/core';
import { Haptics, ImpactStyle } from '@capacitor/haptics';

export const isNative = Capacitor.isNativePlatform();

/** Short vibration feedback; uses the native haptics engine inside the app. */
export function vibrate(strength: 'light' | 'medium' | 'heavy'): void {
  if (isNative) {
    const style = { light: ImpactStyle.Light, medium: ImpactStyle.Medium, heavy: ImpactStyle.Heavy }[strength];
    void Haptics.impact({ style }).catch(() => {});
  } else if ('vibrate' in navigator) {
    try {
      navigator.vibrate({ light: 8, medium: 18, heavy: 35 }[strength]);
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
