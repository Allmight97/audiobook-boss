import { createSignal } from 'solid-js';
import { cleanup, fireEvent, render, screen } from '@solidjs/testing-library';
import { afterEach, describe, expect, it } from 'vitest';
import { CoverImage } from './CoverImage';

describe('CoverImage', () => {
	afterEach(cleanup);

	it('loads lazily, fades in once loaded, and shows a state instead of a broken image', async () => {
		const [src, setSrc] = createSignal<string | null>(null);
		render(() => (
			<CoverImage
				src={src()}
				alt="Book"
				testId="cover"
				missing={<span>No Art</span>}
				failed={<span>Preview failed</span>}
			/>
		));
		expect(screen.getByText('No Art')).toBeInTheDocument();

		setSrc('abb-cover://localhost/one');
		const image = await screen.findByTestId('cover');
		expect(image).toHaveAttribute('loading', 'lazy');
		expect(image).toHaveAttribute('data-state', 'loading');
		await fireEvent.load(image);
		expect(image).toHaveAttribute('data-state', 'ready');

		setSrc('abb-cover://localhost/two');
		const next = await screen.findByTestId('cover');
		expect(next).toHaveAttribute('data-state', 'loading');
		await fireEvent.error(next);
		expect(await screen.findByText('Preview failed')).toBeInTheDocument();
		expect(screen.queryByTestId('cover')).toBeNull();
	});
});
