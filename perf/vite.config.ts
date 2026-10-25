// TEMPORARY: builds the measurement harness (perf/index.html). Removed before merge.
import { resolve } from 'node:path';
import { defineConfig } from 'vite';
import solid from '@solidjs/vite-plugin';

export default defineConfig({
	root: resolve(import.meta.dirname, '..'),
	base: './',
	plugins: [solid({ include: ['/**/*.tsx'] })],
	build: {
		outDir: resolve(import.meta.dirname, 'dist'),
		emptyOutDir: true,
		rollupOptions: { input: resolve(import.meta.dirname, 'index.html') },
	},
});
