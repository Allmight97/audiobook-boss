import { describe, expect, it } from 'vitest';

const fsPromisesSpecifier = 'node:fs/promises';
const { readFile } = (await import(fsPromisesSpecifier)) as {
	readFile(path: string, encoding: 'utf8'): Promise<string>;
};

describe('bootstrap order contract', () => {
	it('installs the frontend log bridge before the Solid app root is imported', async () => {
		const source = await readFile('src/main.tsx', 'utf8');
		const installIndex = source.indexOf('frontendLogBridge.install');
		const appIndex = source.indexOf('ProductionRoot');

		expect(installIndex).toBeGreaterThan(-1);
		expect(appIndex).toBeGreaterThan(-1);
		expect(installIndex).toBeLessThan(appIndex);
	});
});
