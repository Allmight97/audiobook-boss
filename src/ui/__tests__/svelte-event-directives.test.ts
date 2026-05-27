import { describe, expect, it } from 'vitest';

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
	throw new Error('Svelte event directive test requires a Node-compatible test runner');
}

const repoRootUrl = pathToFileURL(`${processRef.cwd()}/`);

const productionSvelteFiles = [
	'src/App.svelte',
	'src/ui/collisionDialog/CollisionDialogIsland.svelte',
	'src/ui/coverArt/CoverArtIsland.svelte',
	'src/ui/encoderPanel/EncoderPanelIsland.svelte',
	'src/ui/fileImport/FileImportIsland.svelte',
	'src/ui/metadataForm/MetadataFormFieldsIsland.svelte',
	'src/ui/metadataLookup/MetadataLookupIsland.svelte',
	'src/ui/outputPanel/OutputPanelIsland.svelte',
	'src/ui/previewAudio/PreviewAudioControls.svelte',
	'src/ui/statusPanel/StatusPanelIsland.svelte',
] as const;

describe('Svelte 5 event attributes', () => {
	it('keeps production components free of deprecated on: event directives', async () => {
		const offenders: string[] = [];

		for (const file of productionSvelteFiles) {
			const source = await readFile(new URL(file, repoRootUrl), 'utf8');
			if (/\son:[A-Za-z-]+\s*=/.test(source)) {
				offenders.push(file);
			}
		}

		expect(offenders).toEqual([]);
	});
});
