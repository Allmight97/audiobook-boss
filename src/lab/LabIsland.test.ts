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
