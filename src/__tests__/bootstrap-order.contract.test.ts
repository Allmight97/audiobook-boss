import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

describe('bootstrap order contract', () => {
	it('installs the frontend log bridge before App.svelte is imported', () => {
		const source = readFileSync(resolve(process.cwd(), 'src/main.ts'), 'utf8');
		const installIndex = source.indexOf("frontendLogBridge.install");
		const appIndex = source.indexOf("App.svelte");

		expect(installIndex).toBeGreaterThan(-1);
		expect(appIndex).toBeGreaterThan(-1);
		expect(installIndex).toBeLessThan(appIndex);
	});
});
