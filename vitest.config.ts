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
					include: ['src/**/*.test.ts', 'src/**/*.test.tsx', 'src/**/*.spec.ts'],
					exclude: ['src/__tests__/bootstrap-order.contract.test.ts'],
					setupFiles: ['./src/test/setup.ts'],
					globals: true,
				},
			},
			{
				extends: true,
				test: {
					name: 'tooling',
					environment: 'node',
					include: ['scripts/**/*.test.ts', 'src/__tests__/bootstrap-order.contract.test.ts'],
				},
			},
		],
	},

	resolve: {
		conditions: ['development', 'browser'],
	},
});
