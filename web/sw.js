// Offline support: every file of the game is cached on install. Pages are
// fetched fresh when online (so updates arrive), everything else comes from
// the cache first. The build replaces __BUILD__, so each release gets its own cache.
const CACHE = 'block-baster-__BUILD__';
const FILES = ['./', './index.html', './block_baster.js', './block_baster_bg.wasm', './manifest.webmanifest', './favicon.svg', './apple-touch-icon.png', './pwa-192.png', './pwa-512.png', './pwa-maskable-512.png'];

self.addEventListener('install', (event) => {
  event.waitUntil(caches.open(CACHE).then((cache) => cache.addAll(FILES)).then(() => self.skipWaiting()));
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) => Promise.all(keys.filter((k) => k.startsWith('block-baster-') && k !== CACHE).map((k) => caches.delete(k))))
      .then(() => self.clients.claim()),
  );
});

self.addEventListener('fetch', (event) => {
  const { request } = event;
  if (request.method !== 'GET' || new URL(request.url).origin !== location.origin) return;
  if (request.mode === 'navigate') {
    event.respondWith(fetch(request).catch(() => caches.match('./index.html')));
    return;
  }
  event.respondWith(caches.match(request, { ignoreSearch: true }).then((hit) => hit || fetch(request)));
});
