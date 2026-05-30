import { ProofUsageError, plan } from '../plan';
import {
	cargoStep,
	frontendTestStep,
	publicApiStripsStep,
	runtimeSteps,
	withRequiredEnv,
} from '../steps';
import type { ProofPlan, ProofStep } from '../types';

function xheAacManualStep(): ProofStep {
	return withRequiredEnv(
		cargoStep(
			'rust-media-manual-xhe-aac',
			'manual xHE-AAC fixture proof',
			'test',
			'-p',
			'audiobook-boss',
			'--test',
			'integration_xhe_aac_fixture_tests',
			'--',
			'--ignored',
		),
		'ABB_XHE_AAC_FIXTURE',
	);
}

function nativeFastpathManualStep(): ProofStep {
	return cargoStep(
		'rust-media-manual-native-fastpath',
		'manual native fastpath fixture proof',
		'test',
		'-p',
		'audiobook-boss',
		'--test',
		'integration_fastpath_parity_tests',
		'--',
		'--ignored',
	);
}

function manualMediaSteps(target: string): ProofStep[] {
	switch (target) {
		case 'all':
			return [xheAacManualStep(), nativeFastpathManualStep()];
		case 'xhe-aac':
			return [xheAacManualStep()];
		case 'native-fastpath':
			return [nativeFastpathManualStep()];
		default:
			throw new ProofUsageError(
				`Unknown media-manual target '${target}'. Use all, xhe-aac, or native-fastpath.`,
			);
	}
}

function rejectExtra(args: string[], message: string): void {
	if (args.length > 0) {
		throw new ProofUsageError(message);
	}
}

function rustContractPlan(args: string[]): ProofPlan {
	rejectExtra(args, 'focus rust contract does not accept extra arguments.');
	return plan(
		'focus.rust.contract',
		'Rust contract proof',
		'focused',
		'Run Rust library contract tests plus Public API Strip assertions.',
		[
			cargoStep(
				'rust-contract-lib',
				'Rust library contract tests',
				'test',
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
	const [testTarget, filter, ...extra] = args;
	if (!testTarget || extra.length > 0) {
		throw new ProofUsageError(
			'Usage: bun scripts/proof/runner.ts focus rust integration <test-target> [filter]',
		);
	}

	const filterArgs = filter ? [filter] : [];
	return plan(
		`focus.rust.integration.${testTarget}`,
		`Rust integration proof: ${testTarget}`,
		'focused',
		'Run one Rust integration test binary with an optional filter.',
		[
			cargoStep(
				'rust-integration-target',
				`Rust integration target ${testTarget}`,
				'test',
				'-p',
				'audiobook-boss',
				'--test',
				testTarget,
				...filterArgs,
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
			cargoStep(
				'rust-lib-filter',
				`Rust library filter ${filter}`,
				'test',
				'-p',
				'audiobook-boss',
				'--lib',
				filter,
			),
		],
	);
}

function rustMediaPlan(args: string[]): ProofPlan {
	rejectExtra(args, 'focus rust media does not accept extra arguments.');
	return plan(
		'focus.rust.media',
		'Rust media fixture proof',
		'focused',
		'Run committed-fixture Rust media integration targets.',
		[
			cargoStep(
				'rust-media-fixtures',
				'Rust committed media fixture targets',
				'test',
				'-p',
				'audiobook-boss',
				'--test',
				'integration_file_list_tests',
				'--test',
				'integration_metadata_tests',
				'--test',
				'integration_metadata_reader_matrix_tests',
				'--test',
				'integration_native_aac_regression_tests',
				'--test',
				'integration_processing_flow_tests',
			),
		],
	);
}

function rustManualMediaPlan(args: string[]): ProofPlan {
	const [manualTarget = 'all', ...extra] = args;
	if (extra.length > 0) {
		throw new ProofUsageError('focus rust media-manual accepts at most one target.');
	}

	return plan(
		`focus.rust.media-manual.${manualTarget}`,
		`Manual Rust media proof: ${manualTarget}`,
		'focused',
		'Run explicit local/manual media fixture proof.',
		manualMediaSteps(manualTarget),
	);
}

function rustPrivatePlan(args: string[]): ProofPlan {
	rejectExtra(args, 'focus rust private does not accept extra arguments.');
	return plan(
		'focus.rust.private',
		'Rust private-cluster proof',
		'focused',
		'Run source-tree library unit tests and internals.',
		[cargoStep('rust-private-lib', 'Rust library tests', 'test', '-p', 'audiobook-boss', '--lib')],
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
		case 'media':
			return rustMediaPlan(rest);
		case 'media-manual':
			return rustManualMediaPlan(rest);
		case 'private':
			return rustPrivatePlan(rest);
		default:
			throw new ProofUsageError(
				'Usage: bun scripts/proof/runner.ts focus rust <lib|integration|contract|private|media|media-manual> ...',
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
			return plan('focus.frontend', 'Frontend proof', 'focused', 'Run TypeScript/Svelte proof.', [
				frontendTestStep(),
			]);
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
		default:
			throw new ProofUsageError(
				'Usage: bun scripts/proof/runner.ts focus <rust|frontend|runtime> ...',
			);
	}
}
