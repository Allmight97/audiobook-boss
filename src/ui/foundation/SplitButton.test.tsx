import { cleanup, fireEvent, render, screen, waitFor } from '@solidjs/testing-library';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { SplitButton } from './SplitButton';

describe('SplitButton', () => {
	afterEach(() => cleanup());

	it('fires main click and toggles the dropdown from the caret', async () => {
		const onMainClick = vi.fn();
		render(() => (
			<SplitButton mainLabel="Import" onMainClick={onMainClick} testId="split-import">
				{({ close }) => <SplitButton.Option onClick={close}>Audible</SplitButton.Option>}
			</SplitButton>
		));

		screen.getByRole('button', { name: 'Import' }).click();
		expect(onMainClick).toHaveBeenCalledOnce();

		const dropdown = screen.getByRole('button', { name: 'Audible' }).closest('.abb-split-dropdown');
		expect(dropdown).not.toHaveClass('open');

		screen.getByRole('button', { name: '▼' }).click();
		await waitFor(() => expect(dropdown).toHaveClass('open'));

		fireEvent.click(document.body);
		await waitFor(() => expect(dropdown).not.toHaveClass('open'));
	});
});
