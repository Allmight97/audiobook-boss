import { describe, expect, it } from 'vitest';
import { populateMetadataFormSingle } from './form';
import { projectTagPreviewValues } from './tags';

describe('tag preview projection', () => {
	it('maps form fields onto tag names and shows the Rust-reported album sort', () => {
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
				'The Mistborn Saga 01 - Mistborn',
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
