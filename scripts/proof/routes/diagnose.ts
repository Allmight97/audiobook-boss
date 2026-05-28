import { ProofUsageError, plan } from '../plan';
import { bashStep, bunStep, cargoStep } from '../steps';
import type { ProofPlan } from '../types';

export function diagnosePlan(args: string[]): ProofPlan {
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
