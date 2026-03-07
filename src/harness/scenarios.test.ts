import { describe, expect, it } from 'vitest';

import {
	findUnmappedHarnessUiPaths,
	getHarnessScenario,
	listHarnessScenarios,
	resolveHarnessScenariosForPaths,
} from './scenarios';

describe('harness scenario routing', () => {
	it('maps metadata surfaces to the metadata-edit scenario', () => {
		const scenarios = resolveHarnessScenariosForPaths([
			'src/ui/metadataLookup.ts',
			'src/ui/metadataForm/MetadataFormFieldsIsland.svelte',
		]);

		expect(scenarios.map((scenario) => scenario.id)).toContain('metadata-edit');
	});

	it('maps status surfaces to the status-processing scenario', () => {
		const scenarios = resolveHarnessScenariosForPaths(['src/ui/statusPanel/logic.ts']);

		expect(scenarios.map((scenario) => scenario.id)).toContain('status-processing');
	});

	it('maps encoder panel surfaces to the output-preview scenario', () => {
		const scenarios = resolveHarnessScenariosForPaths(['src/ui/encoderPanel/logic.ts']);

		expect(scenarios.map((scenario) => scenario.id)).toContain('output-preview');
	});

	it('runs the full suite when shared harness surfaces change', () => {
		const scenarios = resolveHarnessScenariosForPaths(['src/HarnessApp.svelte']);

		expect(scenarios.map((scenario) => scenario.id)).toEqual([
			'file-management',
			'metadata-edit',
			'status-processing',
			'output-preview',
		]);
	});

	it('runs the full suite when harness bootstrap or runner surfaces change', () => {
		expect(
			resolveHarnessScenariosForPaths(['src/harness-main.ts']).map((scenario) => scenario.id),
		).toEqual(['file-management', 'metadata-edit', 'status-processing', 'output-preview']);
		expect(
			resolveHarnessScenariosForPaths(['scripts/harness/verify.ts']).map((scenario) => scenario.id),
		).toEqual(['file-management', 'metadata-edit', 'status-processing', 'output-preview']);
	});

	it('treats backend-only changes as outside the harness lane', () => {
		expect(findUnmappedHarnessUiPaths(['src-tauri/src/commands/audio.rs'])).toEqual([]);
	});

	it('flags uncovered UI-affecting files', () => {
		expect(findUnmappedHarnessUiPaths(['src/ui/unknownPanel.ts'])).toEqual([
			'src/ui/unknownPanel.ts',
		]);
	});

	it('keeps the harness scenarios discoverable', () => {
		expect(listHarnessScenarios().map((scenario) => scenario.id)).toEqual([
			'file-management',
			'metadata-edit',
			'status-processing',
			'output-preview',
		]);
		expect(getHarnessScenario('output-preview').title).toBe('Output Preview');
	});

	it('exposes review controls and verification checks for each scenario', () => {
		for (const scenario of listHarnessScenarios()) {
			expect(scenario.verifyChecks.length).toBeGreaterThan(0);
			expect(scenario.review.controls.length).toBeGreaterThan(0);
			expect(scenario.review.actions.length).toBeGreaterThan(0);
		}
	});
});
