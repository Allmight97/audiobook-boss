import { describe, expect, it } from 'vitest';
import { buildPlan, ProofUsageError } from './proof/catalog';
import { formatCommand } from './proof/format';
import type { ProofPlan, ProofStep } from './proof/types';

function firstCommand(args: string[]): string {
	return formatCommand(buildPlan(args).steps[0]);
}

function cargoNextestSteps(plan: ProofPlan): ProofStep[] {
	return plan.steps.filter((step) => step.tool === 'cargo' && step.args[0] === 'nextest');
}

function assertAudiobookBossTestTargetSelector(step: ProofStep): void {
	const command = formatCommand(step);
	if (!command.startsWith('cargo nextest run -p audiobook-boss ')) {
		return;
	}

	expect(command).toMatch(/^cargo nextest run -p audiobook-boss --(lib|test)\b/);
}

describe('proof route catalog', () => {
	it('runs Rust review proof through Nextest without media execution routes', () => {
		const rustPlan = buildPlan(['review', 'rust']);
		const mainPlan = buildPlan(['review']);

		expect(rustPlan.steps.map(formatCommand)).toEqual([
			'cargo nextest run -p abb-metadata-core -p abb-output-artifact-core -p abb-processing-core -p abb-remote-source-core',
			'cargo nextest run -p audiobook-boss --lib',
			'cargo nextest run -p audiobook-boss --test all_tests',
		]);
		expect(rustPlan.steps[0].preflight?.command).toBe('cargo-nextest');
		expect(rustPlan.steps[0].preflight?.hint).toContain('cargo install cargo-nextest --locked');
		expect(
			mainPlan.steps.some((step) =>
				formatCommand(step).startsWith('cargo nextest run -p abb-metadata-core'),
			),
		).toBe(true);
	});

	it('rejects suspended media execution proof routes', () => {
		expect(() => buildPlan(['review', 'media'])).toThrow(ProofUsageError);
		expect(() => buildPlan(['review', 'full'])).toThrow(ProofUsageError);
	});

	it('exposes rust-target as a non-cleaning diagnostic route', () => {
		const plan = buildPlan(['diagnose', 'rust-target']);

		expect(plan.id).toBe('diagnose.rust-target');
		expect(plan.steps[0].reportOnSuccess).toBe(true);
		expect(formatCommand(plan.steps[0])).toBe('bun scripts/proof/diagnose-rust-target.ts');
	});

	it('renders dependency diagnostics directly through cargo audit', () => {
		const plan = buildPlan(['diagnose', 'deps']);

		expect(plan.id).toBe('diagnose.deps');
		expect(formatCommand(plan.steps[0])).toBe('cargo audit -D warnings');
	});

	it('renders focused Rust library filters with --lib to avoid package-wide fan-out', () => {
		const plan = buildPlan(['focus', 'rust', 'lib', 'metadata_intent_validation_contract']);

		expect(plan.id).toBe('focus.rust.lib.metadata_intent_validation_contract');
		expect(firstCommand(['focus', 'rust', 'lib', 'metadata_intent_validation_contract'])).toBe(
			'cargo nextest run -p audiobook-boss --lib metadata_intent_validation_contract',
		);
	});

	it('renders focused core routes as package-selected Rust proof', () => {
		const plan = buildPlan(['focus', 'core', 'metadata']);

		expect(plan.id).toBe('focus.core.metadata');
		expect(firstCommand(['focus', 'core', 'metadata'])).toBe(
			'cargo nextest run -p abb-metadata-core',
		);
	});

	it('runs review core across all boundary-aligned core crates', () => {
		const plan = buildPlan(['review', 'core']);

		expect(plan.id).toBe('review.core');
		expect(plan.steps.map(formatCommand)).toEqual([
			'cargo nextest run -p abb-metadata-core',
			'cargo nextest run -p abb-output-artifact-core',
			'cargo nextest run -p abb-processing-core',
			'cargo nextest run -p abb-remote-source-core',
		]);
	});

	it('renders Rust contract proof as a focused library route plus Public API Strip assertions', () => {
		const plan = buildPlan(['focus', 'rust', 'contract']);

		expect(plan.id).toBe('focus.rust.contract');
		expect(plan.steps.map((step) => step.id)).toEqual(['rust-contract-lib', 'public-api-strips']);
		expect(formatCommand(plan.steps[0])).toBe(
			'cargo nextest run -p audiobook-boss --lib contract_tests',
		);
	});

	it('renders non-media focused Rust integration proof through the consolidated harness', () => {
		expect(
			firstCommand(['focus', 'rust', 'integration', 'integration_reqwest_resolver_tests']),
		).toBe(
			'cargo nextest run -p audiobook-boss --test all_tests integration_reqwest_resolver_tests',
		);
	});

	it('rejects old route names instead of preserving compatibility aliases', () => {
		expect(() => buildPlan(['rust-contract'])).toThrow(ProofUsageError);
		expect(() => buildPlan(['standard'])).toThrow(ProofUsageError);
		expect(() => buildPlan(['review', 'standard'])).toThrow(ProofUsageError);
		expect(() => buildPlan(['quick'])).toThrow(ProofUsageError);
		expect(() => buildPlan(['focus', 'rust', 'media'])).toThrow(ProofUsageError);
		expect(() => buildPlan(['focus', 'rust', 'private'])).toThrow(ProofUsageError);
		expect(() => buildPlan(['focus', 'rust', 'integration', 'missing_integration_tests'])).toThrow(
			ProofUsageError,
		);
	});

	it('uses tracked frontend tests instead of dirty-worktree discovery for proof routes', () => {
		const plan = buildPlan(['review', 'frontend']);

		expect(plan.steps.length).toBeGreaterThan(1);
		expect(plan.steps.every((step) => step.id.startsWith('frontend-'))).toBe(true);
		const allArgs = plan.steps.flatMap((step) => step.args);
		expect(allArgs).toContain('src/ui/encoderPanel/__tests__/autoResolutionHints.test.ts');
		expect(allArgs.some((arg) => arg.startsWith('scripts/'))).toBe(false);
		for (const step of plan.steps) {
			expect(formatCommand(step)).toMatch(/^bun run test -- src\//);
			expect(step.args).not.toEqual(['run', 'test']);
			expect(step.args.filter((arg) => arg.startsWith('src/')).length).toBeLessThanOrEqual(12);
		}
		const workflowSteps = plan.steps.filter((step) => step.id.startsWith('frontend-workflows-'));
		expect(workflowSteps.length).toBeGreaterThan(1);
		for (const step of workflowSteps) {
			expect(step.args.filter((arg) => arg.startsWith('src/')).length).toBe(1);
		}
	});

	it('keeps every focused Rust cargo test step target-aware for audiobook-boss', () => {
		const focusedRustPlans: ProofPlan[] = [
			buildPlan(['focus', 'rust', 'contract']),
			buildPlan(['focus', 'rust', 'lib', 'metadata_intent_validation_contract']),
			buildPlan(['focus', 'rust', 'integration', 'integration_reqwest_resolver_tests']),
		];

		for (const plan of focusedRustPlans) {
			for (const step of cargoNextestSteps(plan)) {
				assertAudiobookBossTestTargetSelector(step);
			}
		}
	});

	it('keeps focused core cargo test steps target-aware for core packages', () => {
		const focusedCorePlans: ProofPlan[] = [
			buildPlan(['focus', 'core', 'metadata']),
			buildPlan(['focus', 'core', 'output-artifact']),
			buildPlan(['focus', 'core', 'processing']),
			buildPlan(['focus', 'core', 'remote-source']),
		];

		for (const plan of focusedCorePlans) {
			for (const step of plan.steps) {
				expect(formatCommand(step)).toMatch(/^cargo nextest run -p abb-[a-z-]+-core$/);
			}
		}
	});
});
