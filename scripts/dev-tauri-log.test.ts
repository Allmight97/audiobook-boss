import { spawnSync } from 'node:child_process';
import {
	copyFileSync,
	existsSync,
	mkdtempSync,
	mkdirSync,
	readFileSync,
	rmSync,
	writeFileSync,
} from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { expect, it } from 'vitest';

for (const exitCode of [0, 17]) {
	it(`launches bundled Tauri and preserves child exit ${exitCode}`, () => {
		const root = mkdtempSync(path.join(os.tmpdir(), 'abb-dev-launch-'));
		try {
			mkdirSync(path.join(root, 'scripts'));
			mkdirSync(path.join(root, 'bin'));
			copyFileSync(
				path.join(import.meta.dirname, 'dev-tauri-log.sh'),
				path.join(root, 'scripts/dev-tauri-log.sh'),
			);
			writeFileSync(path.join(root, 'scripts/setup.sh'), '#!/bin/bash\nexit 0\n', { mode: 0o755 });
			writeFileSync(path.join(root, 'package.json'), '{}');
			writeFileSync(path.join(root, 'bin/node'), "#!/bin/bash\nprintf '26.10.0\\n'\n", {
				mode: 0o755,
			});
			writeFileSync(path.join(root, 'bin/lsof'), '#!/bin/bash\nexit 1\n', { mode: 0o755 });
			const argsFile = path.join(root, 'launched-args');
			writeFileSync(
				path.join(root, 'bin/bun'),
				`#!/bin/bash
if [[ "$1" == run && "$2" == tauri ]]; then
 printf '%s\\n' "$@" > "$VERIFY_TEST_ARGS"
 exit "$VERIFY_TEST_EXIT"
fi
if [[ "$1" == --version ]]; then printf '1.4.3\\n'; fi
`,
				{ mode: 0o755 },
			);
			const result = spawnSync('bash', ['scripts/dev-tauri-log.sh'], {
				cwd: root,
				encoding: 'utf8',
				env: {
					...process.env,
					PATH: `${root}/bin:${process.env.PATH}`,
					VERIFY_TEST_ARGS: argsFile,
					VERIFY_TEST_EXIT: String(exitCode),
				},
			});
			expect(result.status, result.stderr).toBe(exitCode);
			expect(existsSync(argsFile)).toBe(true);
			expect(readFileSync(argsFile, 'utf8').trim().split('\n')).toEqual([
				'run',
				'tauri',
				'dev',
				'--features',
				'bundled-ffmpeg',
			]);
			expect(readFileSync(path.join(root, '.logs/tauri-dev-summary.md'), 'utf8')).toContain(
				`Exit status: \`${exitCode}\``,
			);
		} finally {
			rmSync(root, { recursive: true, force: true });
		}
	}, 20_000);
}
