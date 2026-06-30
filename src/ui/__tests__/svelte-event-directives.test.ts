import { describe, expect, it } from 'vitest';

const fsPromisesSpecifier = 'node:fs/promises';
const { readFile, readdir } = (await import(fsPromisesSpecifier)) as {
	readFile(path: URL, encoding: 'utf8'): Promise<string>;
	readdir(
		path: URL,
		options: { withFileTypes: true },
	): Promise<
		Array<{
			name: string;
			isDirectory(): boolean;
			isFile(): boolean;
		}>
	>;
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

async function discoverSvelteFiles(
	directoryUrl: URL,
	relativeDirectory: string,
): Promise<string[]> {
	const entries = await readdir(directoryUrl, { withFileTypes: true });
	const files: string[] = [];

	for (const entry of entries.sort((a, b) => a.name.localeCompare(b.name))) {
		const relativePath = `${relativeDirectory}${entry.name}`;
		if (entry.isDirectory()) {
			files.push(
				...(await discoverSvelteFiles(new URL(`${entry.name}/`, directoryUrl), `${relativePath}/`)),
			);
		} else if (entry.isFile() && entry.name.endsWith('.svelte')) {
			files.push(relativePath);
		}
	}

	return files;
}

describe('Svelte 5 event attributes', () => {
	it('keeps production components free of deprecated on: event directives', async () => {
		const offenders: string[] = [];
		const productionSvelteFiles = await discoverSvelteFiles(new URL('src/', repoRootUrl), 'src/');
		expect(productionSvelteFiles).toContain('src/App.svelte');

		for (const file of productionSvelteFiles) {
			const source = await readFile(new URL(file, repoRootUrl), 'utf8');
			if (/\son:[A-Za-z-]+\s*=/.test(source)) {
				offenders.push(file);
			}
		}

		expect(offenders).toEqual([]);
	});
});
