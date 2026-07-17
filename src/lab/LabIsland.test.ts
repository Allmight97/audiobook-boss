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
	});

	it('renders the v3 flat pill kit', () => {
		render(LabIsland);

		expect(screen.getByTestId('pill-primitives-section')).toBeInTheDocument();
		expect(screen.getByRole('button', { name: 'Primary pill' })).toHaveClass(
			'pill',
			'pill-primary',
		);
		expect(screen.getByRole('button', { name: 'Ghost pill' })).toHaveClass('pill', 'pill-ghost');
		expect(screen.getByRole('button', { name: 'Small pill' })).toHaveClass('pill-sm');
		expect(screen.getByRole('button', { name: 'Tiny pill' })).toHaveClass('pill-xs');
	});

	it('renders the field primitive with mixed-value state', () => {
		render(LabIsland);

		const section = screen.getByTestId('field-primitive-section');
		expect(section).toBeInTheDocument();
		expect(section.querySelector('.field label')).toBeTruthy();
		expect(section.querySelector('.field input.mixed')).toBeTruthy();
	});

	it('renders tab-strip and segmented primitives with active states', () => {
		render(LabIsland);

		const tabs = screen.getByTestId('tab-strip-primitive-section');
		expect(tabs.querySelectorAll('.tab-strip .tab').length).toBeGreaterThanOrEqual(3);
		expect(tabs.querySelector('.tab[aria-selected="true"]')).toBeTruthy();

		const seg = screen.getByTestId('segmented-primitive-section');
		expect(seg.querySelectorAll('.segmented button').length).toBeGreaterThanOrEqual(2);
		expect(seg.querySelector('.segmented button.on')).toBeTruthy();
	});

	it('renders the op-card primitive with lanes and a mono log box', () => {
		render(LabIsland);

		const section = screen.getByTestId('op-card-primitive-section');
		const card = section.querySelector('.op-card');
		expect(card).toBeTruthy();
		expect(card?.querySelector('.op-row')).toBeTruthy();
		expect(card?.querySelectorAll('.op-detail .lane').length).toBeGreaterThanOrEqual(2);
		expect(card?.querySelector('.op-log b')).toBeTruthy();
	});

	it('renders and toggles the promoted split-button primitive', async () => {
		render(LabIsland);

		expect(screen.getByTestId('split-button-primitive-section')).toBeInTheDocument();
		expect(screen.getByRole('button', { name: '＋ Import' })).toHaveClass('split-main');

		const caret = screen.getAllByRole('button', { name: 'More options' })[0];
		const option = screen.getByRole('button', { name: 'Menu option' });

		expect(option.closest('.split-dropdown')).not.toHaveClass('open');
		await fireEvent.click(caret);
		expect(option.closest('.split-dropdown')).toHaveClass('open');
		await fireEvent.click(option);
		expect(option.closest('.split-dropdown')).not.toHaveClass('open');
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

	it('renders the constrained modal body specimen', () => {
		render(LabIsland);

		const section = screen.getByTestId('modal-body-scroll-primitive-section');
		expect(section.querySelector('.app-modal-dialog')).toBeTruthy();
		expect(screen.getByTestId('modal-body-scroll-specimen')).toHaveClass('app-modal-body');
		expect(screen.getByText('5. Wind and Truth')).toBeInTheDocument();
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
