import type { Plugin } from 'vite';
import { defineConfig } from 'vite';
import solid from '@solidjs/vite-plugin';

const host = process.env.TAURI_DEV_HOST;

function removeCrossoriginPlugin(): Plugin {
	return {
		name: 'remove-crossorigin',
		transformIndexHtml(html) {
			return html
				.replace(/<script([^>]*?)\scrossorigin(?:="[^"]*")?([^>]*)>/gi, '<script$1$2>')
				.replace(/<link([^>]*?)\scrossorigin(?:="[^"]*")?([^>]*)>/gi, '<link$1$2>');
		},
	};
}

export default defineConfig(async () => ({
	plugins: [solid({ include: ['/**/*.tsx'] }), removeCrossoriginPlugin()],

	clearScreen: false,
	server: {
		port: 1420,
		strictPort: true,
		host: host || false,
		hmr: host
			? {
					protocol: 'ws',
					host,
					port: 1421,
				}
			: undefined,
		watch: {
			ignored: ['**/src-tauri/**'],
		},
	},
	build: {
		modulePreload: false,
		rolldownOptions: {
			output: {
				codeSplitting: false,
			},
		},
	},
}));
