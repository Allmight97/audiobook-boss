import type { OnlineMetadataResult } from '../../lib/generated/tauri';

export function lookupResult(overrides: Partial<OnlineMetadataResult> = {}): OnlineMetadataResult {
	return {
		source: 'audnexus',
		title: 'Lookup Title',
		authors: ['Author One'],
		narrators: ['Narrator One'],
		description: 'Description',
		publishedDate: '2020-07',
		durationSeconds: 3600,
		audibleOnly: false,
		series: null,
		seriesPart: null,
		subseries: null,
		subseriesPart: null,
		cover: 'remote/0/https%3A%2F%2Fexample.com%2Fcover.jpg',
		...overrides,
	};
}
