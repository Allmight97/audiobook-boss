import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import DirectionV3Shell from './DirectionV3Shell.svelte';

describe('DirectionV3Shell', () => {
	it('exposes named selection controls and coherent selection count', async () => {
		render(DirectionV3Shell);

		expect(screen.getByLabelText('Select all books')).toBeInTheDocument();
		expect(screen.getByLabelText('Select The Way of Kings')).toBeInTheDocument();
		expect(screen.queryByTestId('selection-count')).toBeNull();

		await fireEvent.click(screen.getByTestId('fork-multi'));

		expect(screen.getByTestId('selection-count')).toHaveTextContent('3 selected');
	});

	it('renders real metadata tabs with distinct panels', async () => {
		render(DirectionV3Shell);

		expect(screen.getByRole('tab', { name: 'Metadata' })).toHaveAttribute('aria-selected', 'true');
		expect(screen.getByTestId('editor-panel-metadata')).toBeVisible();
		expect(screen.queryByTestId('editor-panel-facts')).toBeNull();

		await fireEvent.click(screen.getByRole('tab', { name: 'Facts' }));

		expect(screen.getByRole('tab', { name: 'Facts' })).toHaveAttribute('aria-selected', 'true');
		expect(screen.getByTestId('editor-panel-facts')).toBeVisible();
		expect(screen.queryByTestId('editor-panel-metadata')).toBeNull();
	});

	it('uses aria-pressed on fork and density controls', () => {
		render(DirectionV3Shell);

		expect(screen.getByTestId('fork-single')).toHaveAttribute('aria-pressed', 'true');
		expect(screen.getByTestId('density-comfortable')).toHaveAttribute('aria-pressed', 'true');
		expect(screen.getByTestId('fork-ops-pinned')).toHaveAttribute('aria-pressed', 'true');
	});

	it('collapses and pins operations panel through coherent modes', async () => {
		render(DirectionV3Shell);

		const opsPanel = screen.getByTestId('ops-panel');
		expect(opsPanel).toHaveClass('open');

		await fireEvent.click(screen.getByTestId('fork-ops-collapsed'));
		expect(opsPanel).not.toHaveClass('open');

		await fireEvent.click(screen.getByTestId('ops-disclosure'));
		expect(opsPanel).toHaveClass('open');
		expect(screen.getByTestId('ops-pin')).toHaveAttribute('aria-pressed', 'false');

		await fireEvent.click(screen.getByTestId('ops-pin'));
		expect(screen.getByTestId('ops-pin')).toHaveAttribute('aria-pressed', 'true');
	});

	it('opens popover from active book, closes on Escape, and restores focus', async () => {
		render(DirectionV3Shell);

		await fireEvent.click(screen.getByRole('button', { name: 'Popover' }));
		const activeButton = screen.getByRole('button', { name: 'Edit metadata for The Way of Kings' });
		await fireEvent.click(activeButton);

		const popover = screen.getByTestId('metadata-popover');
		expect(popover).toBeInTheDocument();

		await fireEvent.keyDown(popover, { key: 'Escape' });
		expect(screen.queryByTestId('metadata-popover')).toBeNull();
		await new Promise<void>((resolve) => queueMicrotask(resolve));
		expect(activeButton).toHaveFocus();
	});

	it('closes popover via labelled close button', async () => {
		render(DirectionV3Shell);

		await fireEvent.click(screen.getByRole('button', { name: 'Popover' }));
		await fireEvent.click(
			screen.getByRole('button', { name: 'Edit metadata for The Way of Kings' }),
		);

		await fireEvent.click(screen.getByTestId('popover-close'));
		expect(screen.queryByTestId('metadata-popover')).toBeNull();
	});

	it('shows batch metadata note for multi selection', async () => {
		render(DirectionV3Shell);

		await fireEvent.click(screen.getByTestId('fork-multi'));
		expect(screen.getByTestId('editor-batch-note')).toBeInTheDocument();
	});

	it('documents desktop shell minimum width', () => {
		render(DirectionV3Shell);

		expect(screen.getByTestId('proto-window')).toHaveStyle({ minWidth: '56.25rem' });
	});
});
