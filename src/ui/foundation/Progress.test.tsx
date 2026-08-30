import { cleanup, render, screen } from '@solidjs/testing-library';
import { afterEach, describe, expect, it } from 'vitest';
import { Progress } from './Progress';

describe('Progress', () => {
	afterEach(() => cleanup());

	it('publishes determinate progress through native semantics', () => {
		render(() => <Progress value={37.4} label="Acquisition progress" fillId="progress-bar" />);
		const bar = screen.getByRole('progressbar', { name: 'Acquisition progress' });
		expect(bar).toHaveAttribute('aria-valuenow', '37');
		expect(document.getElementById('progress-bar')?.style.width).toBe('37.4%');
	});
});
