import { ProofUsageError, plan } from '../plan';
import {
	cargoNextestStep,
	frontendBuildSteps,
	frontendTestSteps,
	quickSteps,
	rustReviewSteps,
	runtimeSteps,
	scriptTestStep,
} from '../steps';
import type { ProofPlan } from '../types';

const CORE_CRATES = [
	'abb-metadata-core',
	'abb-output-artifact-core',
	'abb-processing-core',
	'abb-remote-source-core',
];

function coreSteps() {
	return CORE_CRATES.map((crateName) =>
		cargoNextestStep(`core-${crateName}`, `Core crate ${crateName}`, '-p', crateName),
	);
}

export function reviewPlan(args: string[]): ProofPlan {
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
			return plan(
				'review.rust',
				'Rust review proof',
				'review',
				'Run non-media Rust proof: boundary-aligned core crates plus runtime shell tests.',
				rustReviewSteps(),
			);
		case 'runtime':
			return plan(
				'review.runtime',
				'Runtime boundary proof',
				'review',
				'Run runtime boundary proof.',
				runtimeSteps(),
			);
		case 'frontend':
			return plan(
				'review.frontend',
				'Frontend proof',
				'review',
				'Run frontend Vitest proof in route-owned chunks.',
				frontendTestSteps(),
			);
		case 'core':
			return plan(
				'review.core',
				'Core crate proof',
				'review',
				'Run all boundary-aligned Rust core crate proof.',
				coreSteps(),
			);
		case 'main':
			return plan(
				'review.main',
				'Review proof',
				'review',
				'Run the main non-release review gate with media execution proof suspended.',
				[
					...quickSteps(),
					...rustReviewSteps(),
					scriptTestStep(),
					...frontendTestSteps(),
					...frontendBuildSteps(),
				],
			);
		case 'full':
		case 'media':
			throw new ProofUsageError(
				'Media execution proof is suspended pending issue #341 reassessment. Use review, review quick, review core, review rust, review runtime, or review frontend.',
			);
		default:
			throw new ProofUsageError(
				'Usage: bun scripts/proof/runner.ts review [quick|core|rust|runtime|frontend]',
			);
	}
}
