import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const allowlist = readFileSync(
	path.join(repoRoot, 'scripts/rust-complexity-allowlist.txt'),
	'utf8',
);

const NAMED_SHAPES = ['flat dispatch match:', 'format parser:', 'sequential ? setup'];
const PENDING_REDUCE = '; Reduce at next change';

interface Entry {
	file: string;
	fn: string;
	reason: string;
}

function entries(text: string): Entry[] {
	return text
		.split('\n')
		.filter((line) => line.trim() && !line.startsWith('#'))
		.map((line) => {
			const match = /^(\S+):(\w+)\s+#\s+(.+)$/.exec(line);
			if (!match) throw new Error(`Allowlist line needs <path>:<function>  # <reason>: ${line}`);
			return { file: match[1], fn: match[2], reason: match[3] };
		});
}

function hasNamedReason({ reason }: Entry): boolean {
	return NAMED_SHAPES.some((shape) => reason.startsWith(shape)) || reason.endsWith(PENDING_REDUCE);
}

describe('scripts/rust-complexity-allowlist.txt', () => {
	it('names an existing function for every entry', () => {
		const missing = entries(allowlist).filter(
			({ file, fn }) =>
				!existsSync(path.join(repoRoot, file)) ||
				!new RegExp(`\\bfn ${fn}\\b`).test(readFileSync(path.join(repoRoot, file), 'utf8')),
		);
		expect(missing).toEqual([]);
	});

	it('gives every entry a named shape or a pending Reduce', () => {
		expect(entries(allowlist).filter((entry) => !hasNamedReason(entry))).toEqual([]);
		expect(hasNamedReason({ file: 'x.rs', fn: 'f', reason: 'it is long' })).toBe(false);
	});
});
