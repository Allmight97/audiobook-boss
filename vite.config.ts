import type { Plugin } from 'vite';
import { defineConfig } from 'vite';
import solid from '@solidjs/vite-plugin';

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// tauri://localhost serves assets without CORS headers; crossorigin module tags break WebKit.
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
	// tauri expects a fixed port, fail if that port is not available
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
		// Split chunks created a logic <-> outputPanel import cycle; WebKit rejects it under
		// tauri://localhost. Dev mode serves source modules over http://localhost instead.
		modulePreload: false,
		rolldownOptions: {
			output: {
				codeSplitting: false,
			},
		},
	},
}));
