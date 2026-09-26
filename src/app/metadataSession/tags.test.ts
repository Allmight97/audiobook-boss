import { describe, expect, it } from 'vitest';
import { populateMetadataFormSingle } from './form';
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

	it('maps form fields onto tag names including album and tsoa', () => {
		expect(
			projectTagPreviewValues(
				populateMetadataFormSingle({
					title: 'Mistborn',
					artist: 'Brandon Sanderson',
					composer: 'Michael Kramer',
					series: 'The Mistborn Saga',
					series_part: '1',
					subseries: 'Era 1',
					subseries_part: '2',
					date: '2006',
					genre: 'Fantasy',
				}),
			),
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
