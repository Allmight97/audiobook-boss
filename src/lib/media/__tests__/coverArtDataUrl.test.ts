import { describe, expect, it } from 'vitest';

import { coverArtBytesToDataUrl } from '../coverArtDataUrl';

describe('coverArtBytesToDataUrl', () => {
	it('sniffs jpeg, png, and webp cover art bytes', () => {
		const jpeg = [0xff, 0xd8, 0xff, 0x00];
		const png = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
		const webp = [0x52, 0x49, 0x46, 0x46, 0x00, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50];

		expect(coverArtBytesToDataUrl(jpeg)).toMatch(/^data:image\/jpeg;base64,/);
		expect(coverArtBytesToDataUrl(png)).toMatch(/^data:image\/png;base64,/);
		expect(coverArtBytesToDataUrl(webp)).toMatch(/^data:image\/webp;base64,/);
	});
});
