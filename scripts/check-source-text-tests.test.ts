import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import { sourceTextFindings } from './check-source-text-tests';

const repoRoot = path.resolve(import.meta.dirname, '..');

function trackedTests(): string[] {
	return execFileSync('git', ['ls-files', '*.test.ts', '*.test.tsx', '*.spec.ts', '*.spec.tsx'], {
		cwd: repoRoot,
		encoding: 'utf8',
	})
		.split('\n')
		.filter((file) => /\.(?:test|spec)\.(?:ts|tsx)$/.test(file) && !file.startsWith('vendor/'));
}

describe('check-source-text-tests', () => {
	// Fixture is 7ea2dd4b^:scripts/dev-tauri-log.test.ts, before the spawn rewrite.
	it('flags the vendored source reads from before the spawn rewrite', () => {
		const text = readFileSync(
			path.join(repoRoot, 'scripts/fixtures/dev-tauri-log-7ea2dd4b-parent.txt'),
			'utf8',
		);
		expect(sourceTextFindings('scripts/dev-tauri-log.test.ts', text)).toEqual([
			{
				file: 'scripts/dev-tauri-log.test.ts',
				line: 7,
				message: expect.stringMatching(/implementation source/),
			},
			{
				file: 'scripts/dev-tauri-log.test.ts',
				line: 14,
				message: expect.stringMatching(/package script string/),
			},
		]);
	});

	it('flags Bun.file and a source path held in a variable', () => {
		const text = readFileSync(
			path.join(repoRoot, 'scripts/fixtures/source-text-variable-and-bun.txt'),
			'utf8',
		);
		const findings = sourceTextFindings('scripts/sample.test.ts', text);
		expect(findings.map((finding) => finding.line)).toEqual([2, 3]);
		expect(findings.every((finding) => /implementation source/.test(finding.message))).toBe(true);
	});

	it('has no findings on this checkout', () => {
		const findings = trackedTests().flatMap((file) =>
			sourceTextFindings(file, readFileSync(path.join(repoRoot, file), 'utf8')),
		);
		expect(findings).toEqual([]);
	});
});
