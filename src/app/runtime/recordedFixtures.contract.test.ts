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
