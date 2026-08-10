import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import App from '../../App.svelte';

describe('right column composition', () => {
	it('renders metadata, encoding, Status Panel, and Work Center in order', () => {
		render(App);

		const inputWorkflow = screen.getByRole('region', { name: 'Input and File Order' });
		const metadata = screen.getByRole('region', { name: 'Metadata Manager' });
		const encoding = screen.getByRole('region', { name: 'Encoding, output, and tags' });
		const status = screen.getByRole('button', { name: 'Start Processing' });
		const workCenter = screen.getByRole('heading', { name: 'Work Center' });

		expect(
			inputWorkflow.compareDocumentPosition(metadata) & Node.DOCUMENT_POSITION_FOLLOWING,
		).toBeTruthy();
		expect(
			metadata.compareDocumentPosition(encoding) & Node.DOCUMENT_POSITION_FOLLOWING,
		).toBeTruthy();
		expect(
			encoding.compareDocumentPosition(status) & Node.DOCUMENT_POSITION_FOLLOWING,
		).toBeTruthy();
		expect(
			status.compareDocumentPosition(workCenter) & Node.DOCUMENT_POSITION_FOLLOWING,
		).toBeTruthy();
	});
});
