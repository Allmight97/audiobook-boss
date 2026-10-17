import { execFileSync, spawnSync } from 'node:child_process';
import { copyFileSync, mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { expect, it } from 'vitest';

for (const [staged, working, expected] of [
	['GOOD', 'BAD', 0],
	['BAD', 'GOOD', 11],
] as const) {
	it(`checks staged ${staged} bytes when working bytes are ${working}`, () => {
		const root = mkdtempSync(path.join(os.tmpdir(), 'abb-index-check-'));
		try {
			for (const dir of ['bin', 'scripts', '.githooks']) mkdirSync(path.join(root, dir));
			copyFileSync(
				path.resolve(import.meta.dirname, '../.githooks/pre-commit'),
				path.join(root, '.githooks/pre-commit'),
			);
			writeFileSync(path.join(root, 'scripts/setup.sh'), '#!/bin/bash\nexit 0\n', { mode: 0o755 });
			writeFileSync(path.join(root, 'bin/bun'), '#!/bin/bash\nexit 0\n', { mode: 0o755 });
			writeFileSync(
				path.join(root, 'bin/bunx'),
				`#!/bin/bash
for file in "$@"; do
 if [[ "$file" == *.ts && "$(cat "$file")" == *BAD* ]]; then exit 11; fi
done
`,
				{ mode: 0o755 },
			);
			writeFileSync(path.join(root, '.gitignore'), 'bin/\n');
			const candidate = path.join(root, 'candidate.ts');
			writeFileSync(candidate, `${staged}\n`);
			execFileSync('git', ['init', '-q'], { cwd: root });
			execFileSync('git', ['add', '.'], { cwd: root });
			writeFileSync(candidate, `${working}\n`);
			const result = spawnSync('bash', ['.githooks/pre-commit'], {
				cwd: root,
				encoding: 'utf8',
				env: { ...process.env, PATH: `${root}/bin:${process.env.PATH}` },
			});
			expect(result.status, `${result.stdout}\n${result.stderr}`).toBe(expected);
		} finally {
			rmSync(root, { recursive: true, force: true });
		}
	});
}
