import { describe, expect, it } from 'vitest';
import appShellSource from '../appShell/AppShellIsland.svelte?raw';
import fileListIslandSource from '../fileList/FileListIsland.svelte?raw';
import importIslandSource from '../fileImport/FileImportIsland.svelte?raw';
import appSource from '../../App.svelte?raw';

const fsPromisesSpecifier = 'node:fs/promises';
const { readFile } = (await import(fsPromisesSpecifier)) as {
	readFile(path: URL, encoding: 'utf8'): Promise<string>;
};
const nodeUrlSpecifier = 'node:url';
const { pathToFileURL } = (await import(nodeUrlSpecifier)) as {
	pathToFileURL(path: string): URL;
};
const processRef = (globalThis as { process?: { cwd(): string } }).process;
if (!processRef) {
	throw new Error('file import layout test requires a Node-compatible test runner');
}
const repoRootUrl = pathToFileURL(`${processRef.cwd()}/`);
const globalCss = await readFile(new URL('src/styles.css', repoRootUrl), 'utf8');

function cssRule(source: string, selector: string): string {
	const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
	const match = new RegExp(`${escapedSelector}\\s*\\{(?<body>[^}]*)\\}`, 'm').exec(source);

	expect(match, `Missing CSS rule for ${selector}`).not.toBeNull();
	return match?.groups?.body ?? '';
}

describe('file import layout', () => {
	it('keeps the full-width file area scrollable without the retired column shell', () => {
		expect(appSource).toContain('class="file-area"');
		expect(appSource).not.toContain('leftColumn');
		expect(globalCss).not.toContain('.main-container');
		expect(globalCss).not.toContain('.right-column-wrapper');
		expect(cssRule(fileListIslandSource, '.file-list-content')).toContain('overflow-y: auto;');
	});

	it('keeps file-list keyboard handling local to the file-management region', () => {
		expect(importIslandSource).not.toContain('<svelte:window onkeydown={onFileListKeyDown}');
		expect(fileListIslandSource).toContain('tabindex="0"');
		expect(fileListIslandSource).toContain('onkeydown={onFileListKeyDown}');
	});

	it('leaves the import workflow in place while relocating its controls to app chrome', () => {
		expect(importIslandSource).not.toContain('add-folder-btn');
		expect(importIslandSource).not.toContain('acquire-audiobooks-btn');
		expect(appShellSource).toContain('id="import-files-btn"');
		expect(appShellSource).toContain('id="add-folder-btn"');
		expect(appShellSource).toContain('id="acquire-audiobooks-btn"');
	});
});
