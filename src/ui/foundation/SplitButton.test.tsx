import { flush } from 'solid-js';
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
		fireEvent.click(screen.getByRole('button', { name: 'Import' }));
		flush();
		expect(onMainClick).toHaveBeenCalledOnce();
		const caret = screen.getByRole('button', { name: '▼' });
		expect(caret).toHaveAttribute('aria-expanded', 'false');
		expect(screen.queryByRole('button', { name: 'Audible' })).not.toBeInTheDocument();
		fireEvent.click(caret);
		flush();
		expect(caret).toHaveAttribute('aria-expanded', 'true');
		expect(screen.getByRole('button', { name: 'Audible' })).toBeVisible();
		fireEvent.click(screen.getByRole('button', { name: 'Audible' }));
		flush();
		expect(onSelect).toHaveBeenCalledOnce();
		expect(caret).toHaveAttribute('aria-expanded', 'false');
		expect(screen.queryByRole('button', { name: 'Audible' })).not.toBeInTheDocument();
		fireEvent.click(caret);
		flush();
		fireEvent.click(document.body);
		flush();
		expect(caret).toHaveAttribute('aria-expanded', 'false');
		expect(screen.queryByRole('button', { name: 'Audible' })).not.toBeInTheDocument();
	});
});
