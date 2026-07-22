import { describe, expect, it } from 'vitest';

const fsPromisesSpecifier = 'node:fs/promises';
const { readFile } = (await import(fsPromisesSpecifier)) as {
	readFile(path: string, encoding: 'utf8'): Promise<string>;
};

// Cross-source contract: the frontend purge tombstone may only evict ids the
// backend has already pruned, which holds only while the frontend cap stays
// strictly larger than the backend terminal-retention cap. The two constants
// live in different languages, so paired comments cannot enforce the
// relationship — this test does.
async function constantFrom(filePath: string, pattern: RegExp): Promise<number> {
	const source = await readFile(filePath, 'utf8');
	const match = source.match(pattern);
	if (!match) throw new Error(`Constant not found in ${filePath} (pattern ${pattern})`);
	return Number(match[1]);
}

describe('terminal-retention caps contract', () => {
	it('keeps the frontend purge tombstone strictly larger than the backend terminal cap', async () => {
		const backendCap = await constantFrom(
			'src-tauri/src/work_runtime/state.rs',
			/const TERMINAL_OPERATIONS_CAP: usize = (\d+);/,
		);
		const frontendCap = await constantFrom(
			'src/ui/workCenter/state.svelte.ts',
			/const PURGED_OPERATION_TOMBSTONE_CAP = (\d+);/,
		);
		expect(backendCap).toBeGreaterThan(0);
		expect(frontendCap).toBeGreaterThan(backendCap);
	});
});
