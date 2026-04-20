import { chmodSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { afterEach, describe, expect, it } from 'bun:test';

const tempRoots: string[] = [];

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
	tempRoots.push(repoRoot);

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

	runOrThrow('git', ['init'], { cwd: repoRoot });
	runOrThrow('git', ['config', 'user.name', 'Codex Test'], { cwd: repoRoot });
	runOrThrow('git', ['config', 'user.email', 'codex@example.com'], { cwd: repoRoot });
	runOrThrow('git', ['add', '.'], { cwd: repoRoot });
	runOrThrow('git', ['commit', '-m', 'test: initial fallback fixture'], { cwd: repoRoot });

	return repoRoot;
}

function runFallbackPolicy(
	repoRoot: string,
	options?: { today?: string },
): ReturnType<typeof spawnSync> {
	return spawnSync('bash', ['scripts/check-fallback-policy.sh'], {
		cwd: repoRoot,
		encoding: 'utf8',
		env: {
			...process.env,
			...(options?.today ? { ABB_TODAY: options.today } : {}),
		},
	});
}

afterEach(() => {
	for (const tempRoot of tempRoots.splice(0, tempRoots.length)) {
		rmSync(tempRoot, { force: true, recursive: true });
	}
});

describe('check-fallback-policy.sh', () => {
	it('rejects invalid ABB_TODAY values', () => {
		const repoRoot = createFixtureRepo();
		const result = runFallbackPolicy(repoRoot, { today: '2026-13-01' });

		expect(result.status).toBe(1);
		expect(result.stderr).toContain("Invalid ABB_TODAY value '2026-13-01'");
	});

	it('accepts a valid renewal that extends the sunset', () => {
		const repoRoot = createFixtureRepo('renewal=2026-04-21; reason=fixture extension');
		const result = runFallbackPolicy(repoRoot, { today: '2026-04-20' });

		expect(result.status).toBe(0);
		expect(result.stdout).toContain('[fallback-policy] OK');
	});

	it('rejects malformed register sunset dates', () => {
		const repoRoot = createFixtureRepo();
		const docsPath = path.join(repoRoot, 'docs', 'fallbacks.md');
		writeFileSync(
			docsPath,
			readFileSync(docsPath, 'utf8').replace('| 2026-04-20 |', '| 2026-13-01 |'),
		);

		const result = runFallbackPolicy(repoRoot, { today: '2026-04-20' });

		expect(result.status).toBe(1);
		expect(result.stdout).toContain("has malformed sunset '2026-13-01'");
	});

	it('rejects equal-date renewals', () => {
		const repoRoot = createFixtureRepo('renewal=2026-04-20; reason=fixture extension');
		const result = runFallbackPolicy(repoRoot, { today: '2026-04-20' });

		expect(result.status).toBe(1);
		expect(result.stdout).toContain(
			"renewal date '2026-04-20' does not extend sunset '2026-04-20'",
		);
		expect(result.stdout).not.toContain('expired on 2026-04-20');
	});

	it('rejects backward renewals without treating them as expiry deadlines', () => {
		const repoRoot = createFixtureRepo('renewal=2026-04-19; reason=fixture extension');
		const result = runFallbackPolicy(repoRoot, { today: '2026-04-20' });

		expect(result.status).toBe(1);
		expect(result.stdout).toContain(
			"renewal date '2026-04-19' does not extend sunset '2026-04-20'",
		);
		expect(result.stdout).not.toContain('expired on 2026-04-19');
	});

	it('rejects malformed renewal calendar dates', () => {
		const repoRoot = createFixtureRepo('renewal=2026-13-01; reason=fixture extension');
		const result = runFallbackPolicy(repoRoot, { today: '2026-04-20' });

		expect(result.status).toBe(1);
		expect(result.stdout).toContain("has malformed renewal date '2026-13-01'");
		expect(result.stdout).not.toContain('expired on 2026-13-01');
	});
});
