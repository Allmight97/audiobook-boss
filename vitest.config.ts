import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { svelteTesting } from '@testing-library/svelte/vite';
import solid from 'vite-plugin-solid';

export default defineConfig({
	plugins: [svelte(), svelteTesting(), solid({ include: ['/**/*.tsx'] })],
	test: {
		// Use jsdom for DOM testing (statusPanel, fileList, etc.)
		environment: 'jsdom',

		include: ['src/**/*.test.ts', 'src/**/*.test.tsx', 'src/**/*.spec.ts', 'scripts/**/*.test.ts'],
		environmentMatchGlobs: [['scripts/**', 'node']],

		// Global setup for Tauri mocks
		setupFiles: ['./src/test/setup.ts'],

		// TypeScript support via vite's built-in esbuild
		globals: true,
	},

	resolve: {
		conditions: ['development', 'browser'],
	},
});
