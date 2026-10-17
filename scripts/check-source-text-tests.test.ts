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
	// 7ea2dd4b replaced dev-tauri-log.test.ts reading the shell script and package.json.
	it('flags the source reads at 7ea2dd4b^', () => {
		const list = execFileSync('git', ['ls-tree', '-r', '--name-only', '7ea2dd4b^'], {
			cwd: repoRoot,
			encoding: 'utf8',
		})
			.split('\n')
			.filter((file) => /\.(?:test|spec)\.(?:ts|tsx)$/.test(file) && !file.startsWith('vendor/'));
		const findings = list.flatMap((file) =>
			sourceTextFindings(
				file,
				execFileSync('git', ['show', `7ea2dd4b^:${file}`], { cwd: repoRoot, encoding: 'utf8' }),
			),
		);
		expect(findings).toEqual([
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

	it('has no findings on this checkout', () => {
		const findings = trackedTests().flatMap((file) =>
			sourceTextFindings(file, readFileSync(path.join(repoRoot, file), 'utf8')),
		);
		expect(findings).toEqual([]);
	});
});
