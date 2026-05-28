import type { ProofClassification, ProofPlan, ProofStep } from './types';

export class ProofUsageError extends Error {}

export function plan(
	id: string,
	label: string,
	classification: ProofClassification,
	purpose: string,
	steps: ProofStep[],
): ProofPlan {
	return { classification, id, label, purpose, steps };
}
