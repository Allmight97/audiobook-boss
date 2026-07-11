import { describe, expect, it } from 'vitest';
import appSource from '../../App.svelte?raw';
import metadataManagerSource from '../metadataManager/MetadataManagerIsland.svelte?raw';

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
	throw new Error('right column layout test requires a Node-compatible test runner');
}
const repoRootUrl = pathToFileURL(`${processRef.cwd()}/`);
const globalCss = await readFile(new URL('src/styles.css', repoRootUrl), 'utf8');

function cssRule(source: string, selector: string): string {
	const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
	const match = new RegExp(`${escapedSelector}\\s*\\{(?<body>[^}]*)\\}`, 'm').exec(source);

	expect(match, `Missing CSS rule for ${selector}`).not.toBeNull();
	return match?.groups?.body ?? '';
}

describe('right column sibling layout', () => {
	it('renders only metadata and encoding as right-column sibling zones', () => {
		const metadataIndex = appSource.indexOf(
			'class="panel right-column-panel metadata-manager-panel"',
		);
		const workbenchIndex = appSource.indexOf(
			'class="panel right-column-panel encoding-workbench-panel"',
		);

		expect(metadataIndex).toBeGreaterThan(-1);
		expect(workbenchIndex).toBeGreaterThan(metadataIndex);
		expect(appSource).not.toContain('StatusPanelIsland');
		expect(appSource).not.toContain('WorkCenterIsland');
		expect(appSource).not.toContain('metadata-output-scroll');
		expect(appSource).not.toContain('metadata-output-panel');
	});

	it('keeps right-column overflow at the sibling stack instead of metadata/workbench wrapper', () => {
		const rightColumnRule = cssRule(globalCss, '.right-column-wrapper');
		expect(rightColumnRule).toContain('gap: 0.5rem;');
		expect(rightColumnRule).toContain('overflow-y: auto;');
		expect(globalCss).not.toContain('.metadata-output-scroll');
		expect(globalCss).not.toContain('.metadata-output-panel');
	});

	it('keeps metadata manager as a composition-only island', () => {
		expect(metadataManagerSource).toContain('data-testid="metadata-manager"');
		expect(metadataManagerSource).toContain('<CoverArtIsland');
		expect(metadataManagerSource).toContain('<MetadataFormFieldsIsland');
		expect(metadataManagerSource).toContain('saveMetadataFromUI');
		expect(metadataManagerSource).not.toContain('<EncodingWorkbenchIsland');
	});
});
