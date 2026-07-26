import { describe, expect, it } from 'vitest';

const fsPromisesSpecifier = 'node:fs/promises';
const { readFile } = (await import(fsPromisesSpecifier)) as {
	readFile(path: string, encoding: 'utf8'): Promise<string>;
};

// Cross-source contract: titleBarStyle "Overlay" makes tauri's injected
// drag.js the window's only drag surface, and drag.js needs two capability
// grants to function (start a drag, and toggle maximize on the appbar's
// double-click). The appbar must expose data-tauri-drag-region="deep" so
// clicks on non-clickable children still drag. Nothing else ties these three
// legs together, so a future edit could silently drop any one and leave the
// window undraggable. This test does.
async function readJson(filePath: string): Promise<unknown> {
	const source = await readFile(filePath, 'utf8');
	return JSON.parse(source);
}

describe('titleBarStyle Overlay / drag-capability contract', () => {
	it('grants the two capabilities tauri drag.js needs whenever Overlay is set', async () => {
		const tauriConf = (await readJson('src-tauri/tauri.conf.json')) as {
			app?: { windows?: Array<{ titleBarStyle?: string }> };
		};
		const windows = tauriConf.app?.windows ?? [];
		const usesOverlay = windows.some((windowConfig) => windowConfig.titleBarStyle === 'Overlay');
		expect(usesOverlay).toBe(true);

		const capabilities = (await readJson('src-tauri/capabilities/default.json')) as {
			permissions?: unknown[];
		};
		const permissions = capabilities.permissions ?? [];

		expect(permissions).toContain('core:window:allow-start-dragging');
		expect(permissions).toContain('core:window:allow-internal-toggle-maximize');
	});

	it('pins data-tauri-drag-region="deep" on the app shell appbar', async () => {
		const appShellSource = await readFile('src/ui/appShell/AppShellIsland.svelte', 'utf8');
		expect(appShellSource).toMatch(/data-tauri-drag-region="deep"/);
	});
});
