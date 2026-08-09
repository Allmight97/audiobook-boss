import { describe, expect, it } from 'vitest';

const fsPromisesSpecifier = 'node:fs/promises';
const { readFile } = (await import(fsPromisesSpecifier)) as {
	readFile(path: string, encoding: 'utf8'): Promise<string>;
};

async function constantFrom(filePath: string, pattern: RegExp): Promise<number> {
	const source = await readFile(filePath, 'utf8');
	const match = source.match(pattern);
	if (!match) throw new Error(`Constant not found in ${filePath}`);
	return Number(match[1]);
}

describe('terminal-retention caps contract', () => {
	it('keeps the frontend purge tombstone larger than backend terminal retention', async () => {
		const backendCap = await constantFrom(
			'src-tauri/src/work_runtime/state.rs',
			/const TERMINAL_OPERATIONS_CAP: usize = (\d+);/,
		);
		const frontendCap = await constantFrom(
			'src/ui/workCenter/state.svelte.ts',
			/const PURGED_OPERATION_TOMBSTONE_CAP = (\d+);/,
		);
		expect(frontendCap).toBeGreaterThan(backendCap);
	});
});
