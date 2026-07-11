import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import LabIsland from './LabIsland.svelte';

vi.mock('../ui/metadataLookup', () => ({
	openMetadataLookup: vi.fn(),
}));

vi.mock('../ui/metadataSession', () => ({
	metadataSaveInProgress: { subscribe: vi.fn(() => () => {}) },
}));

describe('LabIsland design fixtures', () => {
	it('renders the promoted badge and pill-size primitives', () => {
		render(LabIsland);

		expect(screen.getByTestId('badge-primitives-section')).toBeInTheDocument();
		expect(screen.getByText('Done')).toHaveClass('app-badge', 'app-badge-ok');
		expect(screen.getByText('Running')).toHaveClass('app-badge-info');
		expect(screen.getByText('Warning')).toHaveClass('app-badge-warn');
		expect(screen.getByText('Queued')).toHaveClass('app-badge-muted');
		expect(screen.getByTestId('pill-size-primitives-section')).toBeInTheDocument();
		expect(screen.getByRole('button', { name: 'Small' })).toHaveClass('btn-pill-sm');
		expect(screen.getByRole('button', { name: 'Extra small' })).toHaveClass('btn-pill-xs');
	});

	it('opens and closes the popover demo through the shared helper', async () => {
		render(LabIsland);
		const trigger = screen.getByRole('button', { name: 'Popover demo' });

		await fireEvent.click(trigger);
		const popover = screen.getByRole('dialog', { name: 'Popover primitive demo' });
		expect(popover).toHaveClass('app-popover');
		await waitFor(() => expect(screen.getByRole('button', { name: 'Close' })).toHaveFocus());

		await fireEvent.keyDown(popover, { key: 'Escape' });
		expect(screen.queryByRole('dialog', { name: 'Popover primitive demo' })).toBeNull();
		await waitFor(() => expect(trigger).toHaveFocus());
	});

	it('renders state fixture cards for expected UI states', () => {
		render(LabIsland);

		expect(screen.getByTestId('state-fixtures-section')).toBeInTheDocument();
		for (const state of [
			'empty',
			'loading',
			'progress',
			'error',
			'success',
			'failure',
			'selected',
			'multi-selected',
		]) {
			expect(screen.getByTestId(`lab-state-${state}`)).toBeInTheDocument();
		}
		expect(screen.getByText('Terminal success')).toBeInTheDocument();
		expect(screen.getByText('Terminal failure')).toBeInTheDocument();
	});

	it('toggles metadata form presets through the real fields island', async () => {
		render(LabIsland);

		await waitFor(() => {
			expect(screen.getByTestId('metadata-fixture-single-clean-populated')).toBeInTheDocument();
		});

		const titleInput = screen.getByLabelText('Book Title') as HTMLInputElement;
		expect(titleInput.value).toBe('The Cartographer of Small Things');
		expect(titleInput).not.toHaveAttribute('data-dirty');
		expect(screen.queryByTestId('meta-title-action')).toBeNull();

		await fireEvent.click(screen.getByTestId('metadata-preset-multi-mixed-dirty-warning'));

		await waitFor(() => {
			expect(screen.getByTestId('metadata-fixture-multi-mixed-dirty-warning')).toBeInTheDocument();
		});

		const revisedTitleInput = screen.getByLabelText('Book Title') as HTMLInputElement;
		const authorInput = screen.getByLabelText('Author') as HTMLInputElement;
		expect(revisedTitleInput.value).toBe('The Shared Title: Revised');
		expect(revisedTitleInput).toHaveAttribute('data-dirty', 'true');
		expect(authorInput).toHaveAttribute('data-mixed', 'true');
		expect(screen.getByTestId('meta-title-action')).toBeInTheDocument();
		expect(
			screen.getByText('Series detected - add Book # (series sequence) for ABS ordering.'),
		).toBeVisible();
	});
});
