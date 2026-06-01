import { execFileSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import type { ProofStep } from './types';

type StepInput = Omit<ProofStep, 'args' | 'command'> & {
	command: [string, ...string[]];
};

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const FRONTEND_TEST_PATTERNS = ['src/**/*.test.ts', 'src/**/*.spec.ts'];
const FRONTEND_TEST_CHUNK_SIZE = 12;
const FRONTEND_WORKFLOW_TEST_CHUNK_SIZE = 1;
const FRONTEND_STATUS_PANEL_TEST_CHUNK_SIZE = 1;
const BASH_PROOF_STEP_TIMEOUT_MS = 600_000;
const BUN_PROOF_STEP_TIMEOUT_MS = 180_000;
const CARGO_PROOF_STEP_TIMEOUT_MS = 1_800_000;

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
		timeoutMs: BASH_PROOF_STEP_TIMEOUT_MS,
		tool: 'bash',
	});
}

export function bunStep(id: string, label: string, ...args: string[]): ProofStep {
	return step({
		command: ['bun', ...args],
		id,
		label,
		timeoutMs: BUN_PROOF_STEP_TIMEOUT_MS,
		tool: 'bun',
	});
}

export function cargoStep(id: string, label: string, ...args: string[]): ProofStep {
	return step({
		command: ['cargo', ...args],
		id,
		label,
		timeoutMs: CARGO_PROOF_STEP_TIMEOUT_MS,
		tool: 'cargo',
	});
}

export function cargoNextestStep(id: string, label: string, ...args: string[]): ProofStep {
	return {
		...cargoStep(id, label, 'nextest', 'run', ...args),
		preflight: {
			args: ['--version'],
			command: 'cargo-nextest',
			hint: 'Install cargo-nextest with `cargo install cargo-nextest --locked` before running full Rust proof.',
		},
	};
}

export function rustReviewStep(): ProofStep {
	return cargoNextestStep(
		'rust-review-core',
		'Rust review suite (core crates only)',
		'-p',
		'abb-metadata-core',
		'-p',
		'abb-output-artifact-core',
		'-p',
		'abb-processing-core',
		'-p',
		'abb-remote-source-core',
	);
}

export function rustRuntimeShellSteps(): ProofStep[] {
	return [
		cargoNextestStep(
			'rust-runtime-lib',
			'Rust runtime shell library tests',
			'-p',
			'audiobook-boss',
			'--lib',
		),
		cargoNextestStep(
			'rust-runtime-integration',
			'Rust runtime shell integration tests',
			'-p',
			'audiobook-boss',
			'--test',
			'all_tests',
		),
	];
}

export function rustReviewSteps(): ProofStep[] {
	return [rustReviewStep(), ...rustRuntimeShellSteps()];
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

function chunks<T>(values: T[], size: number): T[][] {
	const result: T[][] = [];
	for (let index = 0; index < values.length; index += size) {
		result.push(values.slice(index, index + size));
	}
	return result;
}

function frontendGroupStep(
	group: string,
	label: string,
	files: string[],
	index: number,
): ProofStep {
	const suffix = index + 1;
	return bunStep(
		`frontend-${group}-${suffix}`,
		`${label} ${suffix}`,
		'run',
		'test',
		'--',
		...files,
	);
}

export function frontendTestSteps(): ProofStep[] {
	const tests = trackedFiles(FRONTEND_TEST_PATTERNS);
	const groups = [
		{
			id: 'runtime',
			label: 'frontend runtime/type tests',
			files: tests.filter((file) => file.startsWith('src/lib/') || file.startsWith('src/types/')),
			chunkSize: FRONTEND_TEST_CHUNK_SIZE,
		},
		{
			id: 'ui-root',
			label: 'frontend UI root tests',
			files: tests.filter((file) => file.startsWith('src/ui/__tests__/')),
			chunkSize: FRONTEND_TEST_CHUNK_SIZE,
		},
		{
			id: 'workflows',
			label: 'frontend workflow tests',
			files: tests.filter(
				(file) =>
					file.includes('/fileImport/') ||
					file.includes('/metadataLookup/') ||
					file.includes('/outputPanel/') ||
					file.includes('/core/'),
			),
			chunkSize: FRONTEND_WORKFLOW_TEST_CHUNK_SIZE,
		},
		{
			id: 'status-panel',
			label: 'frontend Status Panel tests',
			files: tests.filter((file) => file.startsWith('src/ui/statusPanel/')),
			chunkSize: FRONTEND_STATUS_PANEL_TEST_CHUNK_SIZE,
		},
		{
			id: 'owner-panels',
			label: 'frontend owner panel tests',
			files: tests.filter(
				(file) =>
					file.startsWith('src/ui/') &&
					!file.startsWith('src/ui/__tests__/') &&
					!file.startsWith('src/ui/statusPanel/') &&
					!file.includes('/fileImport/') &&
					!file.includes('/metadataLookup/') &&
					!file.includes('/outputPanel/') &&
					!file.includes('/core/'),
			),
			chunkSize: FRONTEND_TEST_CHUNK_SIZE,
		},
	];
	const covered = new Set(groups.flatMap((group) => group.files));
	if (covered.size !== tests.length) {
		const missing = tests.filter((file) => !covered.has(file));
		throw new Error(`Frontend proof grouping missed tracked tests: ${missing.join(', ')}`);
	}

	return groups.flatMap((group) =>
		chunks(group.files, group.chunkSize).map((files, index) =>
			frontendGroupStep(group.id, group.label, files, index),
		),
	);
}

export function frontendBuildSteps(): ProofStep[] {
	return [
		bunStep('frontend-typecheck', 'frontend TypeScript typecheck', 'x', 'tsc', '--noEmit'),
		bunStep('frontend-vite-build', 'frontend Vite production build', 'x', 'vite', 'build'),
	];
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
