import type { TagField } from './state.svelte';

export type TagRow = {
	field: TagField;
	label: string;
	title: string;
};

export const leftRows: TagRow[] = [
	{
		field: 'title',
		label: 'Title (Book Title)',
		title: "The audiobook's title. Displayed as track/episode name in most players.",
	},
	{
		field: 'album',
		label: 'Album (Book Title)',
		title:
			'Groups all chapters under one album. Plex and Audiobookshelf use this for the book name.',
	},
	{
		field: 'artist',
		label: 'Artist (Author)',
		title: "The book's author. Shows as primary artist in most players.",
	},
	{
		field: 'albumArtist',
		label: 'Album Artist (Author)',
		title: 'Used for library grouping. Keeps all books by an author together.',
	},
	{
		field: 'composer',
		label: 'Composer (Narrator)',
		title:
			'Stores the narrator. Plex shows this in audiobook details; Audiobookshelf displays it as narrator.',
	},
];

export const rightRows: TagRow[] = [
	{
		field: 'series',
		label: 'SERIES (Series)',
		title:
			'Series name tag (series). Written to freeform SERIES (----:com.apple.iTunes:SERIES) for ABS/Plex-compatible scanners.',
	},
	{
		field: 'part',
		label: 'SERIES-PART (Book #)',
		title:
			'Series number tag (series-part). Written to freeform SERIES-PART (----:com.apple.iTunes:SERIES-PART) for ABS/Plex-compatible scanners.',
	},
	{
		field: 'subseries',
		label: 'SERIES (Sub-series)',
		title: 'Secondary series name. Stored as the second entry in the SERIES list for ABS/Plex.',
	},
	{
		field: 'subpart',
		label: 'SERIES-PART (Sub-series #)',
		title: 'Secondary series number. Stored as the second entry in SERIES-PART for ABS/Plex.',
	},
	{
		field: 'tsoa',
		label: 'TSOA (Title Sort Order)',
		title: 'Auto-generated sort key. Forces Plex to sort by series, then book number, then title.',
	},
	{
		field: 'year',
		label: '©day (Publication Date)',
		title: 'Publication date stored as YYYY or YYYY-MM when available.',
	},
	{
		field: 'genre',
		label: 'TCON (Genre)',
		title: 'Genre tag. Used for library filtering and display.',
	},
];

export const compactRows: TagRow[] = [
	{
		field: 'title',
		label: 'Title',
		title: "The audiobook's title.",
	},
	{
		field: 'artist',
		label: 'Author',
		title: "The book's author.",
	},
	{
		field: 'composer',
		label: 'Narrator',
		title: 'The audiobook narrator.',
	},
	{
		field: 'series',
		label: 'Series',
		title: 'Series name used for library grouping.',
	},
	{
		field: 'part',
		label: 'Book #',
		title: 'Series number used for sorting.',
	},
	{
		field: 'genre',
		label: 'Genre',
		title: 'Genre used for library filtering.',
	},
	{
		field: 'year',
		label: 'Year',
		title: 'Publication year or date.',
	},
];
