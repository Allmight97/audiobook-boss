import {
	chmodSync,
	mkdtempSync,
	mkdirSync,
	readFileSync,
	rmSync,
	unlinkSync,
	writeFileSync,
} from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { describe, expect, it } from 'bun:test';

const SYSTEM_BASH = '/bin/bash';
const SYSTEM_GIT = '/usr/bin/git';
const TEST_PATH = '/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin';

function runOrThrow(
	command: string,
	args: string[],
	options: { cwd: string; env?: NodeJS.ProcessEnv },
): string {
	const result = spawnSync(command, args, {
		cwd: options.cwd,
		encoding: 'utf8',
		env: options.env,
	});

	if (result.status !== 0) {
		throw new Error(
			[
				`Command failed: ${command} ${args.join(' ')}`,
				`exit=${String(result.status)}`,
				result.stdout,
				result.stderr,
			]
				.filter(Boolean)
				.join('\n'),
		);
	}

	return result.stdout;
}

function createFixtureRepo(auditStatus = 'OK'): string {
	const repoRoot = mkdtempSync(path.join(os.tmpdir(), 'abb-fallback-policy-'));

	mkdirSync(path.join(repoRoot, 'docs'), { recursive: true });
	mkdirSync(path.join(repoRoot, 'scripts'), { recursive: true });
	mkdirSync(path.join(repoRoot, 'src'), { recursive: true });
	mkdirSync(path.join(repoRoot, 'src-tauri'), { recursive: true });

	const scriptPath = path.join(repoRoot, 'scripts', 'check-fallback-policy.sh');
	writeFileSync(
		scriptPath,
		readFileSync(path.join(import.meta.dir, 'check-fallback-policy.sh'), 'utf8'),
	);
	chmodSync(scriptPath, 0o755);

	writeFileSync(
		path.join(repoRoot, 'src', 'fallback-fixture.sh'),
		[
			'// FALLBACK[FB-999]',
			'// issue=#999',
			'// sunset=2026-04-20',
			'// trigger=test fixture',
			'// observe=test fixture',
			'',
		].join('\n'),
	);
	writeFileSync(
		path.join(repoRoot, 'docs', 'fallbacks.md'),
		[
			'# Fallback Register',
			'',
			'| Marker | Location | Trigger | Observability | Sunset | Issue | Audit Status |',
			'| --- | --- | --- | --- | --- | --- | --- |',
			`| FB-999 | \`src/fallback-fixture.sh\` | test fixture | test fixture | 2026-04-20 | #999 | ${auditStatus} |`,
			'',
		].join('\n'),
	);

	runOrThrow(SYSTEM_GIT, ['init'], { cwd: repoRoot, env: { ...process.env, PATH: TEST_PATH } });
	runOrThrow(SYSTEM_GIT, ['config', 'user.name', 'Codex Test'], { cwd: repoRoot });
	runOrThrow(SYSTEM_GIT, ['config', 'user.email', 'codex@example.com'], { cwd: repoRoot });
	runOrThrow(SYSTEM_GIT, ['add', '.'], { cwd: repoRoot });
	runOrThrow(SYSTEM_GIT, ['commit', '-m', 'test: initial fallback fixture'], { cwd: repoRoot });

	return repoRoot;
}

function runFallbackPolicy(
	repoRoot: string,
	options?: { today?: string },
): { status: number | null; stdout: string; stderr: string } {
	const env = {
		HOME: process.env.HOME ?? '',
		PATH: TEST_PATH,
		TMPDIR: process.env.TMPDIR ?? '',
		...(options?.today ? { ABB_TODAY: options.today } : {}),
	};
	const stdoutPath = path.join(repoRoot, '.fallback-policy.stdout');
	const stderrPath = path.join(repoRoot, '.fallback-policy.stderr');

	for (const outputPath of [stdoutPath, stderrPath]) {
		try {
			unlinkSync(outputPath);
		} catch {}
	}

	const result = spawnSync(
		SYSTEM_BASH,
		[
			'-c',
			'./scripts/check-fallback-policy.sh > .fallback-policy.stdout 2> .fallback-policy.stderr',
		],
		{
			cwd: repoRoot,
			encoding: 'utf8',
			env,
		},
	);

	return {
		status: result.status,
		stdout: readFileSync(stdoutPath, 'utf8'),
		stderr: readFileSync(stderrPath, 'utf8'),
	};
}

describe('check-fallback-policy.sh', () => {
	it('covers invalid dates, renewals, and malformed fallback metadata', () => {
		const assertScenario = (
			auditStatus: string,
			today: string,
			verify: (repoRoot: string, result: ReturnType<typeof runFallbackPolicy>) => void,
		): void => {
			const repoRoot = createFixtureRepo(auditStatus);

			try {
				const result = runFallbackPolicy(repoRoot, { today });
				verify(repoRoot, result);
			} finally {
				rmSync(repoRoot, { force: true, recursive: true });
			}
		};

		assertScenario('OK', '2026-13-01', (_repoRoot, result) => {
			expect(result.status).toBe(1);
			expect(result.stderr).toContain("Invalid ABB_TODAY value '2026-13-01'");
		});

		assertScenario(
			'renewal=2026-04-21; reason=fixture extension',
			'2026-04-20',
			(_repoRoot, result) => {
				expect(result.status).toBe(0);
				expect(result.stdout).toContain('[fallback-policy] OK');
			},
		);

		assertScenario('OK', '2026-04-20', (repoRoot, result) => {
			const docsPath = path.join(repoRoot, 'docs', 'fallbacks.md');
			writeFileSync(
				docsPath,
				readFileSync(docsPath, 'utf8').replace('| 2026-04-20 |', '| 2026-13-01 |'),
			);

			const updatedResult = runFallbackPolicy(repoRoot, { today: '2026-04-20' });
			expect(updatedResult.status).toBe(1);
			expect(updatedResult.stdout).toContain("has malformed sunset '2026-13-01'");
			expect(result.status).toBe(0);
		});

		assertScenario('OK', '2026-04-20', (repoRoot, _result) => {
			const markerPath = path.join(repoRoot, 'src', 'fallback-fixture.sh');
			writeFileSync(
				markerPath,
				readFileSync(markerPath, 'utf8').replace('// sunset=2026-04-20', '// sunset=2026-13-01'),
			);

			const updatedResult = runFallbackPolicy(repoRoot, { today: '2026-04-20' });
			expect(updatedResult.status).toBe(1);
			expect(updatedResult.stdout).toContain("has malformed sunset '2026-13-01'");
		});

		assertScenario(
			'renewal=2026-04-20; reason=fixture extension',
			'2026-04-20',
			(_repoRoot, result) => {
				expect(result.status).toBe(1);
				expect(result.stdout).toContain(
					"renewal date '2026-04-20' does not extend sunset '2026-04-20'",
				);
				expect(result.stdout).not.toContain('expired on 2026-04-20');
			},
		);

		assertScenario(
			'renewal=2026-04-19; reason=fixture extension',
			'2026-04-20',
			(_repoRoot, result) => {
				expect(result.status).toBe(1);
				expect(result.stdout).toContain(
					"renewal date '2026-04-19' does not extend sunset '2026-04-20'",
				);
				expect(result.stdout).not.toContain('expired on 2026-04-19');
			},
		);

		assertScenario(
			'renewal=2026-13-01; reason=fixture extension',
			'2026-04-20',
			(_repoRoot, result) => {
				expect(result.status).toBe(1);
				expect(result.stdout).toContain("has malformed renewal date '2026-13-01'");
				expect(result.stdout).not.toContain('expired on 2026-13-01');
			},
		);
	});
});
