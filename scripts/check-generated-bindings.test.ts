import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

for (const exitCode of ['0', '7']) {
	describe(`binding verification (generator exit ${exitCode})`, () => {
		it('leaves tracked bindings unchanged when generation drifts or fails', () => {
			const root = mkdtempSync(path.join(os.tmpdir(), 'abb-bindings-check-'));
			try {
				for (const directory of ['scripts', 'bin', 'src/lib/generated'])
					mkdirSync(path.join(root, directory), { recursive: true });
				copyFileSync(
					path.join(import.meta.dirname, 'check-generated-bindings.sh'),
					path.join(root, 'scripts/check-generated-bindings.sh'),
				);
				writeFileSync(path.join(root, 'scripts/setup.sh'), '#!/bin/bash\nexit 0\n', {
					mode: 0o755,
				});
				writeFileSync(
					path.join(root, 'bin/bun'),
					`#!/bin/bash
out="${'$'}{3:-src/lib/generated/tauri.ts}"
printf 'generated content\\n' > "$out"
exit ${exitCode}
`,
					{ mode: 0o755 },
				);
				const bindings = path.join(root, 'src/lib/generated/tauri.ts');
				writeFileSync(bindings, 'committed content\n');
				const result = spawnSync('bash', ['scripts/check-generated-bindings.sh'], {
					cwd: root,
					encoding: 'utf8',
					env: { ...process.env, PATH: `${root}/bin:${process.env.PATH}` },
				});
				expect(result.status).not.toBe(0);
				expect(readFileSync(bindings, 'utf8')).toBe('committed content\n');
			} finally {
				rmSync(root, { force: true, recursive: true });
			}
		});
	});
}
