import { describe, expect, it } from 'vitest';
import { EMPTY_METADATA_FORM_PREVIEW_VALUES } from './form';
import { calculateTSOA, projectTagPreviewValues } from './tags';

describe('tag preview projection', () => {
	it('pads series part and skips missing series or title', () => {
		expect(calculateTSOA('The Stormlight Archive', '3', 'Oathbringer')).toBe(
			'The Stormlight Archive 03 - Oathbringer',
		);
		expect(calculateTSOA('Series', '12', 'Finale')).toBe('Series 12 - Finale');
		expect(calculateTSOA('', '1', 'Book')).toBe('');
		expect(calculateTSOA('Series', '1', '')).toBe('');
		expect(calculateTSOA('Series', '0', 'Book')).toBe('Series 00 - Book');
	});

	it('maps preview fields onto tag names including album and tsoa', () => {
		expect(
			projectTagPreviewValues({
				...EMPTY_METADATA_FORM_PREVIEW_VALUES,
				title: 'Mistborn',
				author: 'Brandon Sanderson',
				narrator: 'Michael Kramer',
				series: 'The Mistborn Saga',
				seriesPart: '1',
				subseries: 'Era 1',
				subseriesPart: '2',
				year: '2006',
				genre: 'Fantasy',
			}),
		).toEqual({
			title: 'Mistborn',
			album: 'Mistborn',
			artist: 'Brandon Sanderson',
			albumArtist: 'Brandon Sanderson',
			composer: 'Michael Kramer',
			series: 'The Mistborn Saga',
			part: '1',
			subseries: 'Era 1',
			subpart: '2',
			tsoa: 'The Mistborn Saga 01 - Mistborn',
			year: '2006',
			genre: 'Fantasy',
		});
	});
});
