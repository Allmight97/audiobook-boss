import type { FileListInfo } from '../types/audio';
import type { AudiobookMetadata, OnlineMetadataResult } from '../types/metadata';

export const HARNESS_OUTPUT_DIRECTORY = '/tmp/audiobook-boss-harness';

export const HARNESS_FILE_LIST: FileListInfo = {
	files: [
		{
			path: '/mock/library/Frank Herbert/Dune/01-dune-part-1.mp3',
			size: 15 * 1024 * 1024,
			duration: 300,
			isValid: true,
			bitrate: 64,
			sampleRate: 44100,
			channels: 1,
			format: 'MP3',
			codecLabel: 'MP3',
			selectedDecoder: 'Native MP3',
		},
		{
			path: '/mock/library/Frank Herbert/Dune/02-dune-part-2.mp3',
			size: 20 * 1024 * 1024,
			duration: 400,
			isValid: true,
			bitrate: 64,
			sampleRate: 44100,
			channels: 1,
			format: 'MP3',
			codecLabel: 'MP3',
			selectedDecoder: 'Native MP3',
		},
	],
	totalDuration: 700,
	totalSize: 35 * 1024 * 1024,
	validCount: 2,
	invalidCount: 0,
};

export const HARNESS_METADATA_BY_FILE: Record<string, Partial<AudiobookMetadata>> = {
	'/mock/library/Frank Herbert/Dune/01-dune-part-1.mp3': {
		title: 'Dune',
		album: 'Dune',
		artist: 'Frank Herbert',
		composer: 'Scott Brick',
		series: 'Dune',
		series_part: '1',
		date: '1965',
		genre: 'Science Fiction',
		description: 'Harness fixture for metadata verification.',
	},
	'/mock/library/Frank Herbert/Dune/02-dune-part-2.mp3': {
		title: 'Dune',
		album: 'Dune',
		artist: 'Frank Herbert',
		composer: 'Scott Brick',
		series: 'Dune',
		series_part: '1',
		date: '1965',
		genre: 'Science Fiction',
		description: 'Harness fixture for metadata verification.',
	},
};

export const HARNESS_LOOKUP_RESULTS: OnlineMetadataResult[] = [
	{
		source: 'audnexus',
		sourceId: 'audnexus-dune',
		title: 'Dune',
		authors: ['Frank Herbert'],
		narrators: ['Scott Brick'],
		series: 'Dune',
		seriesPart: '1',
		subseries: undefined,
		subseriesPart: undefined,
		description: 'Set on Arrakis, Dune is the science-fiction epic used in the harness flow.',
		publishedDate: '1965',
		durationSeconds: 36000,
		coverUrl: 'https://example.com/dune-cover.png',
		audibleOnly: false,
	},
];

export const HARNESS_COVER_ART_BYTES = [
	137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 4, 0, 0,
	0, 181, 28, 12, 2, 0, 0, 0, 11, 73, 68, 65, 84, 120, 218, 99, 252, 255, 31, 0, 3, 3, 2, 0, 239,
	161, 160, 242, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];
