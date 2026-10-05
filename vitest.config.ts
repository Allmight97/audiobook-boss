import { defineConfig } from 'vitest/config';
import solid from '@solidjs/vite-plugin';

export default defineConfig({
	plugins: [solid({ include: ['/**/*.tsx'] })],
	test: {
		projects: [
			{
				extends: true,
				test: {
					name: 'frontend',
					environment: 'jsdom',
					// Reuses each worker's jsdom while keeping a fresh context per file.
					pool: 'vmThreads',
					include: ['src/**/*.test.ts', 'src/**/*.test.tsx', 'src/**/*.spec.ts'],
					exclude: [
						'src/__tests__/bootstrap-order.contract.test.ts',
						'src/__tests__/public-api-strips.contract.test.ts',
					],
					setupFiles: ['./src/test/setup.ts'],
					globals: true,
				},
			},
			{
				extends: true,
				test: {
					name: 'tooling',
					environment: 'node',
					include: [
						'scripts/**/*.test.ts',
						'src/__tests__/bootstrap-order.contract.test.ts',
						'src/__tests__/public-api-strips.contract.test.ts',
					],
				},
			},
		],
	},

	resolve: {
		conditions: ['development', 'browser'],
	},
});
