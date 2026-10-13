import '@fontsource/baloo-2/latin-600.css';
import '@fontsource/baloo-2/latin-800.css';
// Display fonts of the higher style tiers; browsers only download them once a style uses them.
import '@fontsource/cinzel-decorative/latin-900.css';
import '@fontsource/lilita-one/latin-400.css';
import '@fontsource/orbitron/latin-900.css';
import '@fontsource/righteous/latin-400.css';
import './styles.css';
import { BlockBasterApp } from './app';
import { Sound } from './audio/sound';

async function start() {
  // Make sure the canvas text uses the bundled font from the first frame.
  await Promise.race([document.fonts?.load("800 32px 'Baloo 2'"), new Promise((r) => setTimeout(r, 1500))]).catch(() => {});

  const params = new URLSearchParams(location.search);
  const seedParam = params.get('seed');
  const seed = seedParam !== null && /^\d+$/.test(seedParam) ? Number(seedParam) : undefined;
  if (params.has('reset')) localStorage.clear();

  const app = new BlockBasterApp(document.getElementById('game') as HTMLCanvasElement, new Sound(), seed);
  // Handy for debugging from the console and for end-to-end tests.
  (window as unknown as { blockBaster: BlockBasterApp }).blockBaster = app;
  document.body.dataset.ready = 'true';
}

void start();

if ('serviceWorker' in navigator && import.meta.env.PROD && !('Capacitor' in window && (window as { Capacitor?: { isNativePlatform?: () => boolean } }).Capacitor?.isNativePlatform?.())) {
  import('virtual:pwa-register').then(({ registerSW }) => registerSW({ immediate: true })).catch(() => {});
}
