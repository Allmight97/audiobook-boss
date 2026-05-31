import { execFileSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import type { ProofStep } from './types';

type StepInput = Omit<ProofStep, 'args' | 'command'> & {
	command: [string, ...string[]];
};

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const FRONTEND_TEST_PATTERNS = ['src/**/*.test.ts', 'src/**/*.spec.ts'];

function step(input: StepInput): ProofStep {
	const [command, ...args] = input.command;
	return { ...input, args, command };
}

export function bashStep(
	id: string,
	label: string,
	scriptPath: string,
	...args: string[]
): ProofStep {
	return step({
		command: ['bash', scriptPath, ...args],
		id,
		label,
		tool: 'bash',
	});
}

export function bunStep(id: string, label: string, ...args: string[]): ProofStep {
	return step({
		command: ['bun', ...args],
		id,
		label,
		tool: 'bun',
	});
}

export function cargoStep(id: string, label: string, ...args: string[]): ProofStep {
	return step({
		command: ['cargo', ...args],
		id,
		label,
		tool: 'cargo',
	});
}

export function withRequiredEnv(stepToWrap: ProofStep, ...requiredEnv: string[]): ProofStep {
	return { ...stepToWrap, requiredEnv };
}

export function generatedBindingsStep(): ProofStep {
	if (process.env.CHECK_BINDINGS_STRICT === '1') {
		return bashStep(
			'generated-bindings',
			'generated binding drift check (strict)',
			'scripts/check-generated-bindings.sh',
			'--mode',
			'verify',
		);
	}

	return bashStep(
		'generated-bindings',
		'generated binding drift check',
		'scripts/check-generated-bindings.sh',
		'--mode',
		'local',
	);
}

export function publicApiStripsStep(): ProofStep {
	return bashStep(
		'public-api-strips',
		'Public API Strip assertions',
		'scripts/check-public-api-strips.sh',
	);
}

export function quickSteps(): ProofStep[] {
	return [
		cargoStep('rust-fmt', 'Rust format check', 'fmt', '--all', '--', '--check'),
		// FALLBACK[FB-018]: Keep Prettier checks for .svelte while Biome Svelte formatting
		// support remains a migration-risky surface for this repo. issue=#219
		// sunset=2026-06-30
		bunStep('frontend-format', 'frontend format check', 'run', 'fmt:check'),
		bunStep('frontend-lint', 'frontend lint check', 'run', 'lint:check'),
		cargoStep(
			'rust-clippy',
			'Rust clippy workspace/all-targets',
			'clippy',
			'--workspace',
			'--all-targets',
			'--',
			'-D',
			'warnings',
		),
		generatedBindingsStep(),
		publicApiStripsStep(),
		bashStep(
			'no-bridge-imports',
			'Tauri bridge import boundary assertion',
			'scripts/check-no-bridge-imports.sh',
		),
		bashStep(
			'no-imperative-dom-runtime',
			'imperative DOM runtime boundary assertion',
			'scripts/check-no-imperative-dom-runtime.sh',
		),
		bashStep(
			'no-legacy-test-contracts',
			'legacy test contract boundary assertion',
			'scripts/check-no-legacy-test-contracts.sh',
		),
		bashStep('fallback-policy', 'fallback policy assertion', 'scripts/check-fallback-policy.sh'),
	];
}

export function scriptTestStep(): ProofStep {
	return bunStep(
		'script-tests',
		'script test subset',
		'run',
		'test',
		'--',
		'scripts/build-app.test.ts',
		'scripts/check-fallback-policy.test.ts',
		'scripts/check-no-bridge-imports.test.ts',
		'scripts/proof-events.test.ts',
		'scripts/proof-executor.test.ts',
		'scripts/proof-routes.test.ts',
		'scripts/resolve-release-dmg.test.ts',
	);
}

function trackedFiles(patterns: readonly string[]): string[] {
	const output = execFileSync('git', ['ls-files', ...patterns], {
		cwd: repoRoot,
		encoding: 'utf8',
	});
	return output
		.split(/\r?\n/)
		.map((line) => line.trim())
		.filter(Boolean);
}

export function frontendTestStep(): ProofStep {
	const tests = trackedFiles(FRONTEND_TEST_PATTERNS);
	return bunStep('frontend-tests', 'tracked frontend Vitest suite', 'run', 'test', '--', ...tests);
}

export function frontendBuildStep(): ProofStep {
	return bunStep('frontend-build', 'production frontend build', 'run', 'build');
}

export function runtimeSteps(): ProofStep[] {
	return [
		generatedBindingsStep(),
		publicApiStripsStep(),
		bunStep(
			'runtime-contract-tests',
			'runtime boundary Vitest contract tests',
			'run',
			'test',
			'--',
			'src/lib/behavior-contract.test.ts',
			'src/lib/tauri-client.test.ts',
			'src/lib/tauri-client.generated-event-bindings.test.ts',
			'src/lib/tauri-public-api.contract.test.ts',
		),
	];
}
