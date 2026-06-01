import { ProofUsageError, plan } from '../plan';
import { cargoNextestStep, frontendTestSteps, publicApiStripsStep, runtimeSteps } from '../steps';
import type { ProofPlan, ProofStep } from '../types';

const RUST_INTEGRATION_HARNESS = 'all_tests';
const RUST_INTEGRATION_MODULES = new Set(['integration_reqwest_resolver_tests']);

function integrationFilter(testModule: string, filter?: string): string {
	return filter ? `${testModule}::${filter}` : testModule;
}

function rustIntegrationHarnessStep(
	id: string,
	label: string,
	testModule: string,
	filter?: string,
	...runnerArgs: string[]
): ProofStep {
	return cargoNextestStep(
		id,
		label,
		'-p',
		'audiobook-boss',
		'--test',
		RUST_INTEGRATION_HARNESS,
		integrationFilter(testModule, filter),
		...runnerArgs,
	);
}

function rejectExtra(args: string[], message: string): void {
	if (args.length > 0) {
		throw new ProofUsageError(message);
	}
}

const CORE_CRATES = {
	metadata: 'abb-metadata-core',
	'output-artifact': 'abb-output-artifact-core',
	processing: 'abb-processing-core',
	'remote-source': 'abb-remote-source-core',
} as const;

type CoreTarget = keyof typeof CORE_CRATES;

function coreTargetIsKnown(target: string): target is CoreTarget {
	return Object.hasOwn(CORE_CRATES, target);
}

function focusedCorePlan(args: string[]): ProofPlan {
	const [target, ...extra] = args;
	if (!target || extra.length > 0 || !coreTargetIsKnown(target)) {
		throw new ProofUsageError(
			'Usage: bun scripts/proof/runner.ts focus core <metadata|output-artifact|processing|remote-source>',
		);
	}

	const crateName = CORE_CRATES[target];
	return plan(
		`focus.core.${target}`,
		`Core crate proof: ${target}`,
		'focused',
		'Run one boundary-aligned Rust core crate without compiling the Tauri/media crate.',
		[cargoNextestStep(`core-${target}`, `Core crate ${crateName}`, '-p', crateName)],
	);
}

function rustContractPlan(args: string[]): ProofPlan {
	rejectExtra(args, 'focus rust contract does not accept extra arguments.');
	return plan(
		'focus.rust.contract',
		'Rust contract proof',
		'focused',
		'Run Rust library contract tests plus Public API Strip assertions.',
		[
			cargoNextestStep(
				'rust-contract-lib',
				'Rust library contract tests',
				'-p',
				'audiobook-boss',
				'--lib',
				'contract_tests',
			),
			publicApiStripsStep(),
		],
	);
}

function rustIntegrationPlan(args: string[]): ProofPlan {
	const [testModule, filter, ...extra] = args;
	if (!testModule || extra.length > 0) {
		throw new ProofUsageError(
			'Usage: bun scripts/proof/runner.ts focus rust integration <test-module> [filter]',
		);
	}
	if (!RUST_INTEGRATION_MODULES.has(testModule)) {
		throw new ProofUsageError(
			`Unknown or suspended Rust integration module: ${testModule}. Add it to the proof route allowlist only after validating it is non-media proof.`,
		);
	}

	return plan(
		`focus.rust.integration.${testModule}`,
		`Rust integration proof: ${testModule}`,
		'focused',
		'Run one module in the consolidated Rust integration harness with an optional filter.',
		[
			rustIntegrationHarnessStep(
				'rust-integration-target',
				`Rust integration module ${testModule}`,
				testModule,
				filter,
			),
		],
	);
}

function rustLibPlan(args: string[]): ProofPlan {
	const [filter, ...extra] = args;
	if (!filter || extra.length > 0) {
		throw new ProofUsageError('Usage: bun scripts/proof/runner.ts focus rust lib <filter>');
	}

	return plan(
		`focus.rust.lib.${filter}`,
		`Rust library proof: ${filter}`,
		'focused',
		'Run one filtered Rust library proof without package-wide test-binary fan-out.',
		[
			cargoNextestStep(
				'rust-lib-filter',
				`Rust library filter ${filter}`,
				'-p',
				'audiobook-boss',
				'--lib',
				filter,
			),
		],
	);
}

function rustPrivatePlan(args: string[]): ProofPlan {
	rejectExtra(args, 'focus rust private does not accept extra arguments.');
	throw new ProofUsageError(
		'focus rust private is suspended pending issue #341 reassessment because it includes audio/media source-tree tests. Use focus core <owner> or focus rust lib <filter>.',
	);
}

function focusedRustPlan(args: string[]): ProofPlan {
	const [target, ...rest] = args;
	switch (target) {
		case 'contract':
			return rustContractPlan(rest);
		case 'integration':
			return rustIntegrationPlan(rest);
		case 'lib':
			return rustLibPlan(rest);
		case 'private':
			return rustPrivatePlan(rest);
		default:
			throw new ProofUsageError(
				'Usage: bun scripts/proof/runner.ts focus rust <lib|integration|contract> ...',
			);
	}
}

export function focusedPlan(args: string[]): ProofPlan {
	const [owner, ...rest] = args;
	switch (owner) {
		case 'frontend':
			if (rest.length > 0) {
				throw new ProofUsageError('focus frontend does not accept extra arguments.');
			}
			return plan(
				'focus.frontend',
				'Frontend proof',
				'focused',
				'Run TypeScript/Svelte proof in route-owned chunks.',
				frontendTestSteps(),
			);
		case 'runtime':
			if (rest.length > 0) {
				throw new ProofUsageError('focus runtime does not accept extra arguments.');
			}
			return plan(
				'focus.runtime',
				'Tauri runtime boundary proof',
				'focused',
				'Run generated binding drift, Public API Strip assertions, and runtime adapter contract tests.',
				runtimeSteps(),
			);
		case 'rust':
			return focusedRustPlan(rest);
		case 'core':
			return focusedCorePlan(rest);
		default:
			throw new ProofUsageError(
				'Usage: bun scripts/proof/runner.ts focus <core|rust|frontend|runtime> ...',
			);
	}
}
