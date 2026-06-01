import { createWriteStream } from 'node:fs';
import { spawn, spawnSync } from 'node:child_process';
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

function preflightFailure(step: ProofStep, options: RunStepOptions, env: EnvLookup): string | null {
	if (!step.preflight) {
		return null;
	}

	const result = spawnSync(step.preflight.command, step.preflight.args, {
		cwd: options.repoRoot,
		encoding: 'utf8',
		env,
		stdio: ['ignore', 'pipe', 'pipe'],
	});

	if (!result.error && result.status === 0) {
		return null;
	}

	const lines = [
		`[proof] Required proof tool is unavailable: ${step.preflight.command}`,
		`[proof] ${step.preflight.hint}`,
	];
	if (result.error) {
		lines.push(`[proof] ${result.error.message}`);
	}
	if (result.stderr?.trim()) {
		lines.push(result.stderr.trim());
	}
	if (result.stdout?.trim()) {
		lines.push(result.stdout.trim());
	}

	return `${lines.join('\n')}\n`;
}

function terminateProcess(child: ReturnType<typeof spawn>): void {
	if (typeof child.pid !== 'number') {
		return;
	}

	try {
		if (process.platform !== 'win32') {
			process.kill(-child.pid, 'SIGTERM');
			return;
		}
		child.kill('SIGTERM');
	} catch {
		child.kill('SIGTERM');
	}
}

function timeoutLabel(timeoutMs: number): string {
	return timeoutMs < 1000 ? `${timeoutMs}ms` : `${(timeoutMs / 1000).toFixed(0)}s`;
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

	const preflightError = preflightFailure(step, options, env);
	if (preflightError) {
		logStream.write(preflightError);
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
				detached: process.platform !== 'win32',
				env,
				stdio: ['ignore', 'pipe', 'pipe'],
			});
			const timeoutMs = step.timeoutMs;
			const timeout = timeoutMs
				? setTimeout(() => {
						logStream.write(`[proof] Step timed out after ${timeoutLabel(timeoutMs)}.\n`);
						terminateProcess(child);
					}, timeoutMs)
				: null;

			child.stdout.on('data', (chunk) => logStream.write(chunk));
			child.stderr.on('data', (chunk) => logStream.write(chunk));
			child.on('error', (error) => {
				if (timeout) clearTimeout(timeout);
				logStream.write(`[proof] Failed to start command: ${error.message}\n`);
				finish(null);
			});
			child.on('close', (code) => {
				if (timeout) clearTimeout(timeout);
				finish(code);
			});
		} catch (error) {
			const message = error instanceof Error ? error.message : String(error);
			logStream.write(`[proof] Failed to start command: ${message}\n`);
			finish(null);
		}
	});

	await closeLogStream(logStream);
	return resultFor(step, options.logPath, startedAt, exitCode);
}
