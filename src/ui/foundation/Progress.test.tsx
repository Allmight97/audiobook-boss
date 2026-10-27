import { cleanup, render, screen } from '@solidjs/testing-library';
import { afterEach, describe, expect, it } from 'vitest';
import { Progress } from './Progress';

describe('Progress', () => {
	afterEach(() => cleanup());

	it('publishes determinate progress through native semantics', () => {
		render(() => <Progress value={37.4} label="Acquisition progress" />);
		const bar = screen.getByRole('progressbar', { name: 'Acquisition progress' });
		expect(bar).toHaveAttribute('aria-valuenow', '37');
		expect(bar).toHaveAttribute('aria-valuemin', '0');
		expect(bar).toHaveAttribute('aria-valuemax', '100');
	});

	it('keeps out-of-range values inside the bar', () => {
		render(() => <Progress value={140} label="Over" />);
		expect(screen.getByRole('progressbar', { name: 'Over' })).toHaveAttribute(
			'aria-valuenow',
			'100',
		);
	});

	it('marks only running work as active, so idle and queued bars stay still', () => {
		render(() => (
			<>
				<Progress value={40} label="Running" active />
				<Progress value={40} label="Queued" />
			</>
		));
		expect(screen.getByRole('progressbar', { name: 'Running' })).toHaveAttribute('data-active');
		expect(screen.getByRole('progressbar', { name: 'Queued' })).not.toHaveAttribute('data-active');
	});
});
