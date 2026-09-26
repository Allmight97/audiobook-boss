import { describe, expect, it } from 'vitest';
import { populateMetadataFormSingle } from './form';
import { projectTagPreviewValues } from './tags';

describe('tag preview projection', () => {
	it.each([
		['The Stormlight Archive', '3', 'Oathbringer', 'The Stormlight Archive 03 - Oathbringer'],
		['Series', '12', 'Finale', 'Series 12 - Finale'],
		['', '1', 'Book', ''],
		['Series', '1', '', ''],
		['Series', '0', 'Book', 'Series 00 - Book'],
	])('sort-album preview for %s #%s %s is %j', (series, part, title, tsoa) => {
		const form = populateMetadataFormSingle({ series, series_part: part, title });
		expect(projectTagPreviewValues(form).tsoa).toBe(tsoa);
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
