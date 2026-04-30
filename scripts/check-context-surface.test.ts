import { chmodSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { describe, expect, it } from 'bun:test';

const SYSTEM_BASH = '/bin/bash';
const TEST_PATH = '/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin';

function writeFile(repoRoot: string, relativePath: string, contents: string): void {
	const filePath = path.join(repoRoot, relativePath);
	mkdirSync(path.dirname(filePath), { recursive: true });
	writeFileSync(filePath, contents);
}

function createContextSurfaceFixture(): string {
	const repoRoot = mkdtempSync(path.join(os.tmpdir(), 'abb-context-surface-'));

	const scriptPath = path.join(repoRoot, 'scripts', 'check-context-surface.sh');
	mkdirSync(path.dirname(scriptPath), { recursive: true });
	writeFileSync(
		scriptPath,
		readFileSync(path.join(import.meta.dir, 'check-context-surface.sh'), 'utf8'),
	);
	chmodSync(scriptPath, 0o755);

	for (const file of [
		'README.md',
		'AGENTS.md',
		'docs/system-map.md',
		'docs/ubiquitous-language.md',
		'docs/fallbacks.md',
		'docs/api-map.md',
		'src/AGENTS.md',
	]) {
		writeFile(repoRoot, file, `${file}\n`);
	}

	writeFile(repoRoot, 'package.json', '{"name":"fixture"}\n');
	writeFile(repoRoot, '.codex/hooks.json', '{}\n');
	writeFile(repoRoot, '.agents/hooks.json', '{}\n');
	writeFile(repoRoot, 'scripts/checks.sh', '#!/usr/bin/env bash\n');

	for (const skill of [
		'audiobook-metadata',
		'decision-alignment',
		'contract-guardrails',
		'job-registry-and-progress',
		'lib-research',
		'path-security-validation',
		'tauri-command-conventions',
	]) {
		writeFile(repoRoot, `.agents/skills/${skill}/SKILL.md`, `# ${skill}\n`);
	}

	for (const hook of [
		'.agents/hooks/common.py',
		'.agents/hooks/session_start.py',
		'.agents/hooks/stop_context_surface.py',
		'.agents/hooks/stop_verification_lane.py',
		'.agents/hooks/stop_ipc_guard.py',
	]) {
		writeFile(repoRoot, hook, '# fixture\n');
	}

	return repoRoot;
}

function runContextSurface(repoRoot: string): {
	status: number | null;
	stdout: string;
	stderr: string;
} {
	const result = spawnSync(SYSTEM_BASH, ['scripts/check-context-surface.sh'], {
		cwd: repoRoot,
		encoding: 'utf8',
		env: {
			HOME: process.env.HOME ?? os.tmpdir(),
			PATH: TEST_PATH,
			TMPDIR: process.env.TMPDIR ?? os.tmpdir(),
		},
	});

	return {
		status: result.status,
		stdout: result.stdout,
		stderr: result.stderr,
	};
}

describe('check-context-surface.sh', () => {
	it('rejects stale PLANS.md planning-surface references', () => {
		const repoRoot = createContextSurfaceFixture();

		try {
			expect(runContextSurface(repoRoot).status).toBe(0);

			writeFile(
				repoRoot,
				'AGENTS.md',
				'Use PLANS.md as a repo-local planning ledger for long-running work.\n',
			);

			const result = runContextSurface(repoRoot);
			expect(result.status).toBe(1);
			expect(result.stdout).toContain('PLANS.md');
		} finally {
			rmSync(repoRoot, { force: true, recursive: true });
		}
	});
});
