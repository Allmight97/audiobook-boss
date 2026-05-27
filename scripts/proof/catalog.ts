import type { ProofClassification, ProofPlan, ProofStep } from './types';
import {
	bashStep,
	bunStep,
	cargoStep,
	frontendBuildStep,
	frontendTestStep,
	publicApiStripsStep,
	quickSteps,
	runtimeSteps,
	scriptTestStep,
} from './steps';

export class ProofUsageError extends Error {}

function plan(
	id: string,
	label: string,
	classification: ProofClassification,
	purpose: string,
	steps: ProofStep[],
): ProofPlan {
	return { classification, id, label, purpose, steps };
}

function manualMediaSteps(target: string): ProofStep[] {
	switch (target) {
		case 'all':
			return [manualMediaSteps('xhe-aac')[0], manualMediaSteps('native-fastpath')[0]];
		case 'xhe-aac':
			return [
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
			];
		case 'native-fastpath':
			return [
				cargoStep(
					'rust-media-manual-native-fastpath',
					'manual native fastpath fixture proof',
					'test',
					'-p',
					'audiobook-boss',
					'--test',
					'integration_fastpath_parity_tests',
					'--',
					'--ignored',
				),
			];
		default:
			throw new ProofUsageError(
				`Unknown media-manual target '${target}'. Use all, xhe-aac, or native-fastpath.`,
			);
	}
}

function focusedRustPlan(args: string[]): ProofPlan {
	const [target, ...rest] = args;
	switch (target) {
		case 'contract':
			if (rest.length > 0) {
				throw new ProofUsageError('focus rust contract does not accept extra arguments.');
			}
			return plan(
				'focus.rust.contract',
				'Rust contract proof',
				'focused',
				'Run Rust library contract tests plus public API strip assertions.',
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
		case 'integration': {
			const [testTarget, filter, ...extra] = rest;
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
		case 'lib': {
			const [filter, ...extra] = rest;
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
		case 'media':
			if (rest.length > 0) {
				throw new ProofUsageError('focus rust media does not accept extra arguments.');
			}
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
		case 'media-manual': {
			const [manualTarget = 'all', ...extra] = rest;
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
		case 'private':
			if (rest.length > 0) {
				throw new ProofUsageError('focus rust private does not accept extra arguments.');
			}
			return plan(
				'focus.rust.private',
				'Rust private-cluster proof',
				'focused',
				'Run source-tree library unit tests and internals.',
				[
					cargoStep(
						'rust-private-lib',
						'Rust library tests',
						'test',
						'-p',
						'audiobook-boss',
						'--lib',
					),
				],
			);
		default:
			throw new ProofUsageError(
				'Usage: bun scripts/proof/runner.ts focus rust <lib|integration|contract|private|media|media-manual> ...',
			);
	}
}

function focusedPlan(args: string[]): ProofPlan {
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
				'Run generated binding drift, public strip assertions, and runtime adapter contract tests.',
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

function reviewPlan(args: string[]): ProofPlan {
	const [target = 'main', ...rest] = args;
	if (rest.length > 0) {
		throw new ProofUsageError(`review ${target} does not accept extra arguments.`);
	}

	switch (target) {
		case 'quick':
			return plan(
				'review.quick',
				'Quick review proof',
				'review',
				'Run static and boundary proof.',
				quickSteps(),
			);
		case 'rust':
			return plan('review.rust', 'Full Rust proof', 'review', 'Run full non-ignored Rust proof.', [
				cargoStep('rust-full', 'Full Rust test suite', 'test'),
			]);
		case 'runtime':
			return plan(
				'review.runtime',
				'Runtime boundary proof',
				'review',
				'Run runtime boundary proof.',
				runtimeSteps(),
			);
		case 'frontend':
			return plan('review.frontend', 'Frontend proof', 'review', 'Run frontend Vitest proof.', [
				frontendTestStep(),
			]);
		case 'main':
			return plan(
				'review.main',
				'Review proof',
				'review',
				'Run the main non-release review gate.',
				[
					...quickSteps(),
					cargoStep('rust-full', 'Full Rust test suite', 'test'),
					scriptTestStep(),
					frontendTestStep(),
					frontendBuildStep(),
				],
			);
		default:
			throw new ProofUsageError(
				'Usage: bun scripts/proof/runner.ts review [quick|rust|runtime|frontend]',
			);
	}
}

function releasePlan(args: string[]): ProofPlan {
	const [target = 'package', ...rest] = args;
	if (target !== 'package' || rest.length > 0) {
		throw new ProofUsageError('Usage: bun scripts/proof/runner.ts release [package]');
	}

	return plan(
		'release.package',
		'Release/package proof',
		'release',
		'Run review proof plus app packaging and AAC decoder contract binary.',
		[
			...reviewPlan([]).steps,
			bunStep('tauri-app-package', 'Tauri app packaging', 'run', 'app:build'),
			cargoStep(
				'aac-decoder-contract-binary',
				'AAC decoder contract binary',
				'run',
				'--manifest-path',
				'src-tauri/Cargo.toml',
				'--bin',
				'verify_aac_decoder_contract',
				'--quiet',
			),
		],
	);
}

function diagnosePlan(args: string[]): ProofPlan {
	const [target, ...rest] = args;
	switch (target) {
		case 'coverage': {
			const [coverageTarget = 'all', ...extra] = rest;
			if (extra.length > 0) {
				throw new ProofUsageError('diagnose coverage accepts at most one target.');
			}
			return plan(
				`diagnose.coverage.${coverageTarget}`,
				`Coverage diagnostic: ${coverageTarget}`,
				'diagnostic',
				'Run explicit coverage diagnostics. Not a release blocker by default.',
				[
					bashStep(
						'coverage',
						`Coverage diagnostic ${coverageTarget}`,
						'scripts/coverage.sh',
						coverageTarget,
					),
				],
			);
		}
		case 'deps':
			if (rest.length > 0) {
				throw new ProofUsageError('diagnose deps does not accept extra arguments.');
			}
			return plan(
				'diagnose.deps',
				'Dependency hygiene proof',
				'diagnostic',
				'Run dependency audit.',
				[bunStep('dependency-audit', 'dependency audit', 'run', 'check:deps')],
			);
		case 'timing':
			return plan(
				'diagnose.timing',
				'Build timing diagnostic',
				'diagnostic',
				'Run Cargo build timing diagnostics.',
				[cargoStep('cargo-build-timing', 'Cargo build timings', 'build', '--timings', ...rest)],
			);
		default:
			throw new ProofUsageError(
				'Usage: bun scripts/proof/runner.ts diagnose <coverage|deps|timing> ...',
			);
	}
}

export function buildPlan(args: string[]): ProofPlan {
	const [category = 'review', ...rest] = args;
	switch (category) {
		case 'focus':
			return focusedPlan(rest);
		case 'review':
			return reviewPlan(rest);
		case 'release':
			return releasePlan(rest);
		case 'diagnose':
			return diagnosePlan(rest);
		default:
			throw new ProofUsageError(
				'Unknown proof category. Use focus, review, release, diagnose, or --help.',
			);
	}
}

export function formatCommand(stepToFormat: ProofStep): string {
	return [stepToFormat.command, ...stepToFormat.args].join(' ');
}
