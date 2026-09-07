import { cleanup, fireEvent, render, screen } from '@solidjs/testing-library';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { SplitButton } from './SplitButton';

describe('SplitButton', () => {
	afterEach(() => cleanup());

	it('dispatches the main action and closes visible options after selection or an outside click', async () => {
		const onMainClick = vi.fn();
		const onSelect = vi.fn();
		render(() => (
			<SplitButton mainLabel="Import" onMainClick={onMainClick} testId="split-import">
				{({ close }) => (
					<SplitButton.Option
						onClick={() => {
							onSelect();
							close();
						}}
					>
						Audible
					</SplitButton.Option>
				)}
			</SplitButton>
		));
		await fireEvent.click(screen.getByRole('button', { name: 'Import' }));
		expect(onMainClick).toHaveBeenCalledOnce();
		const caret = screen.getByRole('button', { name: '▼' });
		expect(caret).toHaveAttribute('aria-expanded', 'false');
		expect(screen.queryByRole('button', { name: 'Audible' })).not.toBeInTheDocument();
		await fireEvent.click(caret);
		expect(caret).toHaveAttribute('aria-expanded', 'true');
		expect(screen.getByRole('button', { name: 'Audible' })).toBeVisible();
		await fireEvent.click(screen.getByRole('button', { name: 'Audible' }));
		expect(onSelect).toHaveBeenCalledOnce();
		expect(caret).toHaveAttribute('aria-expanded', 'false');
		expect(screen.queryByRole('button', { name: 'Audible' })).not.toBeInTheDocument();
		await fireEvent.click(caret);
		await fireEvent.click(document.body);
		expect(caret).toHaveAttribute('aria-expanded', 'false');
		expect(screen.queryByRole('button', { name: 'Audible' })).not.toBeInTheDocument();
	});
});
