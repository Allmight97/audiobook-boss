import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { assertionCount, ratchetFindings, skippedTestCount } from './check-assertion-ratchet';

const roots: string[] = [];

afterEach(() => {
	for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

function git(root: string, args: string[]): string {
	return execFileSync('git', args, { cwd: root, encoding: 'utf8' });
}

function repo(): string {
	const root = mkdtempSync(path.join(os.tmpdir(), 'assertion-ratchet-'));
	roots.push(root);
	git(root, ['init', '-b', 'main']);
	git(root, ['config', 'user.email', 'ratchet@example.com']);
	git(root, ['config', 'user.name', 'Ratchet']);
	return root;
}

function commit(root: string, files: Record<string, string | null>): string {
	for (const [name, text] of Object.entries(files)) {
		const absolute = path.join(root, name);
		if (text === null) {
			git(root, ['rm', '-f', name]);
			continue;
		}
		mkdirSync(path.dirname(absolute), { recursive: true });
		writeFileSync(absolute, text);
		git(root, ['add', name]);
	}
	git(root, ['commit', '-m', 'change']);
	return git(root, ['rev-parse', 'HEAD']).trim();
}

function findings(root: string, base: string, head: string) {
	const show = (rev: string, file: string): string | null => {
		try {
			return git(root, ['show', `${rev}:${file}`]);
		} catch {
			return null;
		}
	};
	const rows = git(root, ['diff', '--name-status', '-M', base, head])
		.split('\n')
		.filter(Boolean)
		.map((line) => {
			const [status, from, to] = line.split('\t');
			return { status, from, to: to ?? from };
		});
	return ratchetFindings(root, base, head, rows, show);
}

describe('check-assertion-ratchet', () => {
	it('counts assertion lines and ignores comments', () => {
		expect(assertionCount('expect(1).toBe(1);\nassert(true);\nconst x = 1;')).toBe(2);
		expect(assertionCount('// expect(real).toBe(1);\n/* assert(false); */')).toBe(0);
		expect(skippedTestCount("it.skip('a', () => {});\n// it.todo('b')")).toBe(1);
		expect(skippedTestCount('const sample = "it.skip(\'a\')";')).toBe(0);
		expect(skippedTestCount("it.skipIf(true)('a', () => expect(1).toBe(1));")).toBe(1);
		expect(skippedTestCount("test.skipIf(false)('a', () => {});")).toBe(1);
		expect(skippedTestCount("xit('a', () => {});\nxdescribe('s', () => {});")).toBe(2);
	});

	it('flags a dropped assertion and exempts a test deleted with its code', () => {
		const root = repo();
		const base = commit(root, {
			'scripts/build-app.test.ts': 'expect(1).toBe(1);\nexpect(2).toBe(2);\n',
			'scripts/gone.test.ts': "import { gone } from './gone';\nexpect(gone).toBe(1);\n",
			'scripts/gone.ts': 'export const gone = 1;\n',
		});
		const head = commit(root, {
			'scripts/build-app.test.ts': 'expect(1).toBe(1);\n',
			'scripts/gone.test.ts': null,
			'scripts/gone.ts': null,
		});
		expect(findings(root, base, head)).toEqual([
			{
				file: 'scripts/build-app.test.ts',
				message: expect.stringMatching(/assertions dropped from 2 to 1/),
			},
		]);
	});

	it('flags a commented-out assertion, a skip, a todo, and a rename off the test suffix', () => {
		const root = repo();
		const base = commit(root, {
			'src/live.test.ts': "it('keeps', () => expect(1).toBe(1));\n",
			'src/soon.test.ts': 'expect(1).toBe(1);\n',
			'src/later.test.ts': 'expect(1).toBe(1);\n',
			'src/renamed.test.ts': 'expect(1).toBe(1);\n',
		});
		const head = commit(root, {
			'src/live.test.ts': "it.skip('keeps', () => expect(1).toBe(1));\n",
			'src/soon.test.ts': '// expect(1).toBe(1);\n',
			'src/later.test.ts': "test.todo('later');\nexpect(1).toBe(1);\n",
			'src/renamed.unit.ts': 'expect(1).toBe(1);\n',
			'src/renamed.test.ts': null,
		});
		const found = findings(root, base, head);
		expect(found.map((finding) => finding.file).sort()).toEqual([
			'src/later.test.ts',
			'src/live.test.ts',
			'src/renamed.test.ts',
			'src/soon.test.ts',
		]);
		expect(found.find((finding) => finding.file === 'src/live.test.ts')?.message).toMatch(
			/skipped or todo tests increased from 0 to 1/,
		);
		expect(found.find((finding) => finding.file === 'src/soon.test.ts')?.message).toMatch(
			/assertions dropped from 1 to 0/,
		);
		expect(found.find((finding) => finding.file === 'src/renamed.test.ts')?.message).toMatch(
			/renamed to src\/renamed\.unit\.ts/,
		);
	});

	it('flags skipIf and xit added to a test', () => {
		const root = repo();
		const base = commit(root, {
			'src/a.test.ts': "it('a', () => expect(1).toBe(1));\n",
		});
		const head = commit(root, {
			'src/a.test.ts':
				"it.skipIf(true)('a', () => expect(1).toBe(1));\nxit('b', () => expect(2).toBe(2));\n",
		});
		expect(findings(root, base, head)).toEqual([
			{
				file: 'src/a.test.ts',
				message: expect.stringMatching(/skipped or todo tests increased from 0 to 2/),
			},
		]);
	});
});
