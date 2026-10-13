import type { CapacitorConfig } from '@capacitor/cli';

const config: CapacitorConfig = {
  appId: 'io.github.noartem.blockbaster',
  appName: 'Block Baster',
  webDir: 'dist',
  backgroundColor: '#2f43ab',
  android: {
    // Everything ships inside the APK; the game never needs the network.
    allowMixedContent: false,
  },
  plugins: {
    SystemBars: {
      insetsHandling: 'css',
      initialViewportFitValueHint: 'cover',
      // Light status bar icons on our dark blue background.
      style: 'DARK',
    },
  },
};

export default config;
