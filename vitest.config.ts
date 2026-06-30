import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { svelteTesting } from '@testing-library/svelte/vite';

export default defineConfig({
	plugins: [svelte(), svelteTesting()],
	test: {
		// Use jsdom for DOM testing (statusPanel, fileList, etc.)
		environment: 'jsdom',

		include: ['src/**/*.test.ts', 'src/**/*.spec.ts', 'scripts/**/*.test.ts'],
		environmentMatchGlobs: [['scripts/**', 'node']],

		// Global setup for Tauri mocks
		setupFiles: ['./src/test/setup.ts'],

		// TypeScript support via vite's built-in esbuild
		globals: true,
	},

	resolve: {
		conditions: ['browser'],
	},
});
