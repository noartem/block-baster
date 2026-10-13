import type { BlockBasterApp } from '../src/app';

declare global {
  interface Window {
    blockBaster: BlockBasterApp;
  }
}
