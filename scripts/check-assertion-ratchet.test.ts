import path from 'node:path';
import { describe, expect, it } from 'vitest';
import { assertionCount, ratchetFindings } from './check-assertion-ratchet';
import { execFileSync } from 'node:child_process';

const repoRoot = path.resolve(import.meta.dirname, '..');

function show(rev: string, file: string): string | null {
	try {
		return execFileSync('git', ['show', `${rev}:${file}`], { cwd: repoRoot, encoding: 'utf8' });
	} catch {
		return null;
	}
}

function rows(base: string, head: string) {
	return execFileSync('git', ['diff', '--name-status', '-M', base, head], {
		cwd: repoRoot,
		encoding: 'utf8',
	})
		.split('\n')
		.filter(Boolean)
		.map((line) => {
			const [status, from, to] = line.split('\t');
			return { status, from, to: to ?? from };
		});
}

describe('check-assertion-ratchet', () => {
	it('counts assertion lines', () => {
		expect(assertionCount('expect(1).toBe(1);\nassert(true);\nconst x = 1;')).toBe(2);
	});

	// main..833ce9c9 dropped one build-app assertion; c6092b9a later restored it.
	// unmet-lanes.test.ts was removed with scripts/unmet-lanes.ts.
	it('flags main..833ce9c9 and exempts a test deleted with its code', () => {
		const findings = ratchetFindings(repoRoot, 'main', '833ce9c9', rows('main', '833ce9c9'), show);
		expect(findings).toEqual([
			{
				file: 'scripts/build-app.test.ts',
				message: expect.stringMatching(/assertions dropped from 51 to 50/),
			},
		]);
		expect(findings.map((finding) => finding.file)).not.toContain('scripts/unmet-lanes.test.ts');
	});
});
