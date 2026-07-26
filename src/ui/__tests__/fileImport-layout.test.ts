import { describe, expect, it } from 'vitest';
import fileListIslandSource from '../fileList/FileListIsland.svelte?raw';
import importIslandSource from '../fileImport/FileImportIsland.svelte?raw';
import inputWorkflowSource from '../leftColumn/InputWorkflowPanel.svelte?raw';

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
	it('constrains the input file list to the scrollable work area', () => {
		expect(inputWorkflowSource).toContain(
			'class="left-column-panel input-workflow input-workflow-panel flex flex-col gap-2 mb-2"',
		);
		expect(cssRule(globalCss, '.input-panel')).toContain('overflow: hidden;');
		expect(cssRule(globalCss, '.left-column-wrapper')).toContain('overflow: hidden;');
		const inputWorkflowRule = cssRule(globalCss, '.input-workflow-panel');
		expect(inputWorkflowRule).toContain('flex: 1 1 auto;');
		expect(inputWorkflowRule).toContain('min-height: 0;');
		expect(inputWorkflowRule).toContain('overflow: hidden;');
		expect(cssRule(fileListIslandSource, '.file-list-content')).toContain('overflow-y: auto;');
	});

	it('keeps file-list keyboard handling local to the file-management region', () => {
		expect(importIslandSource).not.toContain('<svelte:window onkeydown={onFileListKeyDown}');
		expect(fileListIslandSource).toContain('tabindex="0"');
		expect(fileListIslandSource).toContain('onkeydown={onFileListKeyDown}');
	});
});
