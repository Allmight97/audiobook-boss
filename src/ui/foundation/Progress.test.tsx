import { cleanup, render, screen } from '@solidjs/testing-library';
import { afterEach, describe, expect, it } from 'vitest';
import { Progress } from './Progress';

describe('Progress', () => {
	afterEach(() => cleanup());

	it('slides a full-width fill so the rounded end stays round', () => {
		render(() => <Progress value={37.4} label="Acquisition progress" fillId="progress-bar" />);
		const bar = screen.getByRole('progressbar', { name: 'Acquisition progress' });
		expect(bar).toHaveAttribute('aria-valuenow', '37');
		expect(bar.classList.contains('is-active')).toBe(false);
		const fill = document.getElementById('progress-bar');
		expect(fill?.style.transform).toBe('translateX(-62.6%)');
		expect(fill?.style.width).toBe('');
	});

	it('shimmers only while the bar is active', () => {
		render(() => <Progress value={10} active label="Working" />);
		expect(
			screen.getByRole('progressbar', { name: 'Working' }).classList.contains('is-active'),
		).toBe(true);
	});
});
