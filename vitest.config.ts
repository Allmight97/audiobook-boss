import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
	plugins: [svelte()],
	test: {
		// Use jsdom for DOM testing (statusPanel, fileList, etc.)
		environment: 'jsdom',

		// Include source files for coverage
		include: ['src/**/*.test.ts', 'src/**/*.spec.ts'],

		// Coverage configuration
		coverage: {
			provider: 'v8',
			reporter: ['text', 'html', 'lcov'],
			reportsDirectory: './coverage/typescript',
			include: ['src/**/*.ts', 'src/**/*.svelte'],
			exclude: [
				'src/**/*.test.ts',
				'src/**/*.spec.ts',
				'src/lib/mocks.ts', // Dev mocks, not production code
			],
			// Thresholds - start modest, increase as coverage improves
			thresholds: {
				// These are reporting thresholds, not blocking thresholds
				// Set to 0 initially since we're starting from 0% coverage
				statements: 0,
				branches: 0,
				functions: 0,
				lines: 0,
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
