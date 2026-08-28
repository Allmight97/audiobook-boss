import { cleanup, render, screen } from '@solidjs/testing-library';
import { afterEach, describe, expect, it } from 'vitest';
import { Lab } from './Lab';

describe('Solid design lab', () => {
	afterEach(() => {
		cleanup();
		delete document.documentElement.dataset.density;
	});

	it('flips html density from the lab switch', () => {
		render(() => <Lab />);
		expect(document.documentElement.dataset.density).toBeUndefined();
		screen.getByRole('button', { name: 'Compact' }).click();
		expect(document.documentElement.dataset.density).toBe('compact');
		screen.getByRole('button', { name: 'Comfortable' }).click();
		expect(document.documentElement.dataset.density).toBeUndefined();
	});
});
