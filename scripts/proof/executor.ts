import { createWriteStream } from 'node:fs';
import { spawn } from 'node:child_process';
import { formatCommand } from './format';
import type { ProofStep, ProofStepResult } from './types';

type EnvLookup = Record<string, string | undefined>;

export type RunStepOptions = {
	env?: EnvLookup;
	logPath: string;
	repoRoot: string;
};

function closeLogStream(logStream: ReturnType<typeof createWriteStream>): Promise<void> {
	return new Promise((resolve) => logStream.end(resolve));
}

function resultFor(
	step: ProofStep,
	logPath: string,
	startedAt: number,
	exitCode: number | null,
): ProofStepResult {
	const status = exitCode === 0 ? 'passed' : 'failed';
	return { ...step, durationMs: Date.now() - startedAt, exitCode, logPath, status };
}

function missingRequiredEnv(step: ProofStep, env: EnvLookup): string[] {
	return step.requiredEnv?.filter((name) => !env[name]) ?? [];
}

export async function runStep(step: ProofStep, options: RunStepOptions): Promise<ProofStepResult> {
	const startedAt = Date.now();
	const env = options.env ?? process.env;
	const logStream = createWriteStream(options.logPath);
	logStream.write(`$ ${formatCommand(step, env)}\n\n`);

	const missingEnv = missingRequiredEnv(step, env);
	if (missingEnv.length > 0) {
		logStream.write(`[proof] Missing required environment variable(s): ${missingEnv.join(', ')}\n`);
		await closeLogStream(logStream);
		return resultFor(step, options.logPath, startedAt, null);
	}

	const exitCode = await new Promise<number | null>((resolve) => {
		let settled = false;
		const finish = (code: number | null) => {
			if (settled) {
				return;
			}
			settled = true;
			resolve(code);
		};

		try {
			const child = spawn(step.command, step.args, {
				cwd: options.repoRoot,
				env,
				stdio: ['ignore', 'pipe', 'pipe'],
			});

			child.stdout.on('data', (chunk) => logStream.write(chunk));
			child.stderr.on('data', (chunk) => logStream.write(chunk));
			child.on('error', (error) => {
				logStream.write(`[proof] Failed to start command: ${error.message}\n`);
				finish(null);
			});
			child.on('close', finish);
		} catch (error) {
			const message = error instanceof Error ? error.message : String(error);
			logStream.write(`[proof] Failed to start command: ${message}\n`);
			finish(null);
		}
	});

	await closeLogStream(logStream);
	return resultFor(step, options.logPath, startedAt, exitCode);
}
