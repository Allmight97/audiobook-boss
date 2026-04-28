import { describe, expect, it } from 'bun:test';

import {
	SCENARIO_TIMEOUT_MS,
	buildScenarioTimeoutMessage,
	classifyConsoleMessage,
	parseArgs,
} from './verify';

describe('parseArgs', () => {
	it('defaults to changed mode when no args are provided', () => {
		expect(parseArgs([])).toEqual({ mode: 'changed' });
	});

	it('accepts explicit changed mode', () => {
		expect(parseArgs(['--changed'])).toEqual({ mode: 'changed' });
	});

	it('parses one or more scenario ids', () => {
		expect(parseArgs(['--scenario', 'status-processing'])).toEqual({
			mode: 'scenario',
			scenarioIds: ['status-processing'],
		});

		expect(parseArgs(['--scenario', 'status-processing', '--scenario', 'file-management'])).toEqual(
			{
				mode: 'scenario',
				scenarioIds: ['status-processing', 'file-management'],
			},
		);
	});
});

describe('classifyConsoleMessage', () => {
	it('allows fallback-marked warnings', () => {
		expect(
			classifyConsoleMessage({
				type: 'warning',
				text: 'FALLBACK[FB-001] using cached result',
			}),
		).toBe('ignored');
	});

	it('fails generic warnings and errors', () => {
		expect(
			classifyConsoleMessage({
				type: 'warning',
				text: 'Missing metadata during preview',
			}),
		).toBe('fatal');
		expect(
			classifyConsoleMessage({
				type: 'error',
				text: 'Unhandled exception',
			}),
		).toBe('fatal');
	});
});

describe('buildScenarioTimeoutMessage', () => {
	it('reports scenario timeout failures with scenario identity and timeout budget', () => {
		expect(buildScenarioTimeoutMessage('status-processing')).toBe(
			`Harness scenario status-processing timed out after ${SCENARIO_TIMEOUT_MS}ms.`,
		);
	});
});
