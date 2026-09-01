import { cleanup, render, screen } from '@solidjs/testing-library';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ScenarioSwitcher } from './ScenarioSwitcher';

afterEach(() => {
	cleanup();
});

describe('mock scenario switcher', () => {
	it('exposes every required fixture scenario on the real-app chrome overlay', async () => {
		const onSelect = vi.fn();
		render(() => <ScenarioSwitcher current="empty" onSelect={onSelect} />);

		expect(screen.getByTestId('ui-mock-scenario-switcher')).toBeTruthy();
		for (const scenario of [
			'empty',
			'files-loaded',
			'encoding-in-progress',
			'error',
			'audible-logged-out',
		]) {
			expect(screen.getByTestId(`ui-mock-scenario-${scenario}`)).toBeTruthy();
		}

		screen.getByTestId('ui-mock-scenario-files-loaded').click();
		expect(onSelect).toHaveBeenCalledWith('files-loaded');
	});
});
