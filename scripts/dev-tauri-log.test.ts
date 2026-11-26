import { spawn, spawnSync } from 'node:child_process';
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

const TAURI_LAUNCH = `if [[ "$1" == run && "$2" == tauri ]]; then
 printf '%s\\n' "$@" > "$VERIFY_TEST_ARGS"
 printf '%s' "$RUST_LOG" > "$VERIFY_TEST_RUST_LOG"
 exit "$VERIFY_TEST_EXIT"
fi`;

const TAURI_UNTIL_INTERRUPTED = `if [[ "$1" == run && "$2" == tauri ]]; then
 trap 'printf "child stopped\\n"; exit 130' INT
 printf 'child started\\n'
 : > "$VERIFY_TEST_STARTED"
 for _ in {1..300}; do sleep 0.1; done
fi`;

function fixture(tauri: string, extraBins: Record<string, string> = {}) {
	const root = mkdtempSync(path.join(os.tmpdir(), 'abb-dev-launch-'));
	mkdirSync(path.join(root, 'scripts'));
	mkdirSync(path.join(root, 'bin'));
	copyFileSync(
		path.join(import.meta.dirname, 'dev-tauri-log.sh'),
		path.join(root, 'scripts/dev-tauri-log.sh'),
	);
	writeFileSync(path.join(root, 'scripts/setup.sh'), '#!/bin/bash\nexit 0\n', { mode: 0o755 });
	writeFileSync(path.join(root, 'package.json'), '{}');
	const bins: Record<string, string> = {
		node: "printf '26.10.0\\n'",
		lsof: 'exit 1',
		bun: `${tauri}\nif [[ "$1" == --version ]]; then printf '1.4.3\\n'; fi`,
		...extraBins,
	};
	for (const [name, body] of Object.entries(bins)) {
		writeFileSync(path.join(root, 'bin', name), `#!/bin/bash\n${body}\n`, { mode: 0o755 });
	}
	const env = {
		...process.env,
		PATH: `${root}/bin:${process.env.PATH}`,
		VERIFY_TEST_ARGS: path.join(root, 'launched-args'),
		VERIFY_TEST_RUST_LOG: path.join(root, 'launched-rust-log'),
		VERIFY_TEST_STARTED: path.join(root, 'launched'),
		ABB_DEV_RUST_LOG: '',
		ABB_DEV_USE_EXISTING_RUST_LOG: '',
		VERIFY_TEST_EXIT: '0',
	};
	return {
		root,
		env,
		summary: () => readFileSync(path.join(root, '.logs/tauri-dev-summary.md'), 'utf8'),
		log: () => readFileSync(path.join(root, '.logs/tauri-dev.log'), 'utf8'),
		cleanup: () => rmSync(root, { recursive: true, force: true }),
	};
}

for (const exitCode of [0, 17]) {
	it(`launches bundled Tauri and preserves child exit ${exitCode}`, () => {
		const run = fixture(TAURI_LAUNCH);
		try {
			const result = spawnSync('bash', ['scripts/dev-tauri-log.sh'], {
				cwd: run.root,
				encoding: 'utf8',
				env: { ...run.env, VERIFY_TEST_EXIT: String(exitCode) },
			});
			expect(result.status, result.stderr).toBe(exitCode);
			expect(existsSync(run.env.VERIFY_TEST_ARGS)).toBe(true);
			expect(readFileSync(run.env.VERIFY_TEST_ARGS, 'utf8').trim().split('\n')).toEqual([
				'run',
				'tauri',
				'dev',
				'--features',
				'bundled-ffmpeg',
			]);
			expect(readFileSync(run.env.VERIFY_TEST_RUST_LOG, 'utf8').split(',')).toEqual(
				expect.arrayContaining([
					'audiobook_boss=info',
					'abb_engine=info',
					'abb_media_pipeline=info',
				]),
			);
			expect(run.summary()).toContain(`Exit status: \`${exitCode}\``);
			expect(run.summary()).not.toContain('Log capture did not drain cleanly');
		} finally {
			run.cleanup();
		}
	}, 20_000);
}

it('marks the session indeterminate when the log tee fails to write', () => {
	const run = fixture(TAURI_LAUNCH, { tee: 'cat > /dev/null\nexit 1' });
	try {
		const result = spawnSync('bash', ['scripts/dev-tauri-log.sh'], {
			cwd: run.root,
			encoding: 'utf8',
			env: run.env,
		});
		expect(result.status, result.stderr).toBe(0);
		expect(run.summary()).toContain('Health: `indeterminate`');
		expect(run.summary()).toContain('Log capture did not drain cleanly');
	} finally {
		run.cleanup();
	}
}, 20_000);

it('drains the log and writes the summary after Ctrl-C', async () => {
	const run = fixture(TAURI_UNTIL_INTERRUPTED);
	try {
		const child = spawn('bash', ['scripts/dev-tauri-log.sh'], {
			cwd: run.root,
			env: run.env,
			detached: true,
			stdio: 'ignore',
		});
		const exited = new Promise<number | null>((resolve) => child.on('exit', resolve));
		await new Promise<void>((resolve, reject) => {
			const deadline = Date.now() + 10_000;
			const poll = setInterval(() => {
				if (existsSync(run.env.VERIFY_TEST_STARTED)) {
					clearInterval(poll);
					resolve();
				} else if (Date.now() > deadline) {
					clearInterval(poll);
					reject(new Error('the fake Tauri child never started'));
				}
			}, 50);
		});
		process.kill(-(child.pid ?? 0), 'SIGINT');
		expect(await exited).toBe(130);
		expect(run.log()).toContain('child started');
		expect(run.log()).toContain('child stopped');
		expect(run.summary()).toContain('Exit status: `130`');
		expect(run.summary()).not.toContain('Log capture did not drain cleanly');
	} finally {
		run.cleanup();
	}
}, 20_000);
