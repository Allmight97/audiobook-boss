import { describe, expect, it } from 'vitest';
import sessionFresh from '../../lib/generated/fixtures/session-fresh.json';
import sessionScenarios from '../../lib/generated/fixtures/session-scenarios.json';

describe('recorded session scenarios', () => {
	const updateKeys = Object.keys(sessionFresh).toSorted();

	it.each(sessionScenarios.map((scenario) => [scenario.name, scenario.steps] as const))(
		'%s records every step with the SessionUpdate shape',
		(_name, steps) => {
			expect(steps.length).toBeGreaterThan(0);
			for (const step of steps) {
				expect(Object.keys(step.update).toSorted()).toEqual(updateKeys);
			}
		},
	);
});

const PLATFORM_SPECIFIC_VALUES: Record<string, RegExp> = {
	'os error number': /os error \d+/,
	'home directory': /\/(?:Users|home)\/[^/"\\\s]+\//,
	'temp path': /\/tmp\/|\/var\/folders\/|\/private\/var\//,
};

const recordedFixtureFiles = import.meta.glob('../../lib/generated/fixtures/*.json', {
	eager: true,
	query: '?raw',
	import: 'default',
});

describe('recorded fixtures', () => {
	it('finds the committed fixture files', () => {
		expect(Object.keys(recordedFixtureFiles).length).toBeGreaterThan(0);
	});

	it.each(Object.entries(recordedFixtureFiles))(
		'%s holds no platform-specific value',
		(_file, text) => {
			for (const [name, pattern] of Object.entries(PLATFORM_SPECIFIC_VALUES)) {
				expect(text.match(pattern)?.[0], name).toBeUndefined();
			}
		},
	);
});
