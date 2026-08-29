import type { Plugin } from 'vite';
import { defineConfig } from 'vite';
import tailwindcss from '@tailwindcss/vite';
import solid from 'vite-plugin-solid';

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

// https://vitejs.dev/config/
export default defineConfig(async () => ({
	plugins: [tailwindcss(), solid({ include: ['/**/*.tsx'] }), removeCrossoriginPlugin()],

	// Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
	//
	// 1. prevent vite from obscuring rust errors
	clearScreen: false,
	// 2. tauri expects a fixed port, fail if that port is not available
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
			// 3. tell vite to ignore app-external Rust
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
