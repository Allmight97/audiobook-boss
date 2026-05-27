import { describe, expect, it } from 'bun:test';
import { buildPlan, formatCommand, ProofUsageError } from './proof/catalog';

function firstCommand(args: string[]): string {
	return formatCommand(buildPlan(args).steps[0]);
}

describe('proof route catalog', () => {
	it('renders focused Rust library filters with --lib to avoid package-wide fan-out', () => {
		const plan = buildPlan(['focus', 'rust', 'lib', 'metadata_intent_validation_contract']);

		expect(plan.id).toBe('focus.rust.lib.metadata_intent_validation_contract');
		expect(firstCommand(['focus', 'rust', 'lib', 'metadata_intent_validation_contract'])).toBe(
			'cargo test -p audiobook-boss --lib metadata_intent_validation_contract',
		);
	});

	it('renders Rust contract proof as a focused library route plus public strip assertions', () => {
		const plan = buildPlan(['focus', 'rust', 'contract']);

		expect(plan.id).toBe('focus.rust.contract');
		expect(plan.steps.map((step) => step.id)).toEqual(['rust-contract-lib', 'public-api-strips']);
		expect(formatCommand(plan.steps[0])).toBe('cargo test -p audiobook-boss --lib contract_tests');
	});

	it('renders focused Rust integration proof with an explicit test target', () => {
		expect(
			firstCommand(['focus', 'rust', 'integration', 'integration_metadata_tests', 'reads_track']),
		).toBe('cargo test -p audiobook-boss --test integration_metadata_tests reads_track');
	});

	it('rejects old route names instead of preserving compatibility aliases', () => {
		expect(() => buildPlan(['rust-contract'])).toThrow(ProofUsageError);
		expect(() => buildPlan(['standard'])).toThrow(ProofUsageError);
		expect(() => buildPlan(['review', 'standard'])).toThrow(ProofUsageError);
		expect(() => buildPlan(['quick'])).toThrow(ProofUsageError);
	});
});
