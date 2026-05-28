import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import { runStep } from './proof/executor';
import type { ProofStep } from './proof/types';

function tempRepo(): string {
	return mkdtempSync(path.join(os.tmpdir(), 'abb-proof-executor-'));
}

describe('proof step executor', () => {
	it('turns missing executables into ordinary failed proof results', async () => {
		const repoRoot = tempRepo();
		try {
			const logPath = path.join(repoRoot, 'missing-command.log');
			const step: ProofStep = {
				args: [],
				command: '__abb_missing_proof_command__',
				id: 'missing-command',
				label: 'missing command',
				tool: 'bash',
			};

			const result = await runStep(step, { logPath, repoRoot });

			expect(result.status).toBe('failed');
			expect(result.exitCode).toBeNull();
			expect(result.logPath).toBe(logPath);
			expect(readFileSync(logPath, 'utf8')).toContain('[proof] Failed to start command:');
		} finally {
			rmSync(repoRoot, { force: true, recursive: true });
		}
	});

	it('fails missing required env before spawning the command', async () => {
		const repoRoot = tempRepo();
		try {
			const logPath = path.join(repoRoot, 'missing-env.log');
			const step: ProofStep = {
				args: ['test'],
				command: 'cargo',
				id: 'env-preflight',
				label: 'env preflight',
				requiredEnv: ['ABB_XHE_AAC_FIXTURE'],
				tool: 'cargo',
			};

			const result = await runStep(step, { env: {}, logPath, repoRoot });

			expect(result.status).toBe('failed');
			expect(result.exitCode).toBeNull();
			expect(readFileSync(logPath, 'utf8')).toContain(
				'Missing required environment variable(s): ABB_XHE_AAC_FIXTURE',
			);
		} finally {
			rmSync(repoRoot, { force: true, recursive: true });
		}
	});
});
