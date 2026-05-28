import { describe, expect, it } from 'vitest';
import { buildPlan, formatCommand, ProofUsageError } from './proof/catalog';
import type { ProofPlan, ProofStep } from './proof/types';

function firstCommand(args: string[]): string {
	return formatCommand(buildPlan(args).steps[0]);
}

function cargoTestSteps(plan: ProofPlan): ProofStep[] {
	return plan.steps.filter((step) => step.tool === 'cargo' && step.args[0] === 'test');
}

function assertAudiobookBossTestTargetSelector(step: ProofStep): void {
	const command = formatCommand(step);
	if (!command.startsWith('cargo test -p audiobook-boss ')) {
		return;
	}

	expect(command).toMatch(/^cargo test -p audiobook-boss --(lib|test)\b/);
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

	it('keeps every focused Rust cargo test step target-aware for audiobook-boss', () => {
		const focusedRustPlans: ProofPlan[] = [
			buildPlan(['focus', 'rust', 'contract']),
			buildPlan(['focus', 'rust', 'lib', 'metadata_intent_validation_contract']),
			buildPlan(['focus', 'rust', 'integration', 'integration_metadata_tests', 'reads_track']),
			buildPlan(['focus', 'rust', 'private']),
			buildPlan(['focus', 'rust', 'media']),
			buildPlan(['focus', 'rust', 'media-manual', 'all']),
			buildPlan(['focus', 'rust', 'media-manual', 'xhe-aac']),
			buildPlan(['focus', 'rust', 'media-manual', 'native-fastpath']),
		];

		for (const plan of focusedRustPlans) {
			for (const step of cargoTestSteps(plan)) {
				assertAudiobookBossTestTargetSelector(step);
			}
		}
	});
});
