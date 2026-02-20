import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import HarnessApp from '../HarnessApp.svelte';

describe('HarnessApp fixture-driven rendering', () => {
	it('renders default harness fixtures for input and lookup panels', () => {
		render(HarnessApp);

		expect(screen.getByRole('heading', { name: 'Harness: Input' })).toBeInTheDocument();
		expect(
			screen.getByRole('heading', { name: 'Harness: Metadata Lookup Modal' }),
		).toBeInTheDocument();
		expect(screen.getByTestId('metadata-lookup-modal')).toBeInTheDocument();
	});

	it('honors fixture overrides for user-facing harness composition', () => {
		render(HarnessApp, {
			props: {
				fixture: {
					labels: {
						inputPanelTitle: 'Fixture Input Lane',
					},
					islands: {
						metadataLookup: { enabled: false },
					},
				},
			},
		});

		expect(screen.getByRole('heading', { name: 'Fixture Input Lane' })).toBeInTheDocument();
		expect(screen.queryByTestId('harness-metadata-lookup')).not.toBeInTheDocument();
		expect(screen.queryByTestId('metadata-lookup-modal')).not.toBeInTheDocument();
	});
});
