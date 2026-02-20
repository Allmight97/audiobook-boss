import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { svelteTesting } from '@testing-library/svelte/vite';

export default defineConfig({
	plugins: [svelte(), svelteTesting()],
	test: {
		// Use jsdom for DOM testing (statusPanel, fileList, etc.)
		environment: 'jsdom',

		// Include source files for coverage
		include: ['src/**/*.test.ts', 'src/**/*.spec.ts'],

		// Coverage configuration
		coverage: {
			provider: 'istanbul',
			reporter: ['text', 'html', 'lcov'],
			reportsDirectory: './coverage/typescript',
			include: ['src/**/*.ts', 'src/**/*.svelte'],
			exclude: ['src/**/*.test.ts', 'src/**/*.spec.ts'],
			// Ratchet thresholds — baseline measured Feb 2026, rounded down ~3pt
			// Increase as coverage improves; do not lower without justification
			thresholds: {
				statements: 60,
				branches: 42,
				functions: 56,
				lines: 61,
			},
		},

		// Global setup for Tauri mocks
		setupFiles: ['./src/test/setup.ts'],

		// TypeScript support via vite's built-in esbuild
		globals: true,
	},

	// Resolve aliases to match the main vite config if needed
	resolve: {
		conditions: ['browser'],
		alias: {
			// Add any path aliases here if needed
		},
	},
});
