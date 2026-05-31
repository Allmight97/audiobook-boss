import { ProofUsageError, plan } from '../plan';
import {
	cargoNextestStep,
	frontendBuildStep,
	frontendTestStep,
	quickSteps,
	runtimeSteps,
	scriptTestStep,
} from '../steps';
import type { ProofPlan } from '../types';

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
			return plan('review.rust', 'Full Rust proof', 'review', 'Run full non-ignored Rust proof.', [
				cargoNextestStep('rust-full', 'Full Rust test suite (nextest)'),
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
					cargoNextestStep('rust-full', 'Full Rust test suite (nextest)'),
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
