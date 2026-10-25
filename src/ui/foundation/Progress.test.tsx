import { cleanup, render, screen } from '@solidjs/testing-library';
import { afterEach, describe, expect, it } from 'vitest';
// The reduced-motion rule lives in the stylesheet, which the component test cannot compute.
// biome-ignore lint/correctness/noNodejsModules: reads that stylesheet.
import { readFileSync } from 'node:fs';
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

	it('turns the shimmer off under reduced motion', () => {
		const chromeCss = readFileSync('src/ui/foundation/internal/chrome.css', 'utf8');
		const reduced = chromeCss.slice(chromeCss.indexOf('@media (prefers-reduced-motion: reduce)'));
		expect(chromeCss).toContain('.abb-progress.is-active .abb-progress-fill::after');
		expect(reduced).toContain('animation: none');
		expect(reduced).toContain('transition: none');
	});
});
