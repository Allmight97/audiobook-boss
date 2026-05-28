import { formatCommand } from './format';
import { ProofUsageError } from './plan';
import { diagnosePlan } from './routes/diagnose';
import { focusedPlan } from './routes/focus';
import { releasePlan } from './routes/release';
import { reviewPlan } from './routes/review';
import type { ProofPlan } from './types';

export { formatCommand, ProofUsageError };

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
