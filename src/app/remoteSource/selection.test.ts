import { describe, expect, it } from 'vitest';
import type { RemoteRelease, RemoteTitle } from '../../types/remoteSource';
import {
	selectedRemoteTitleSummaryText,
	toggledRemoteTitleSelection,
	toggledSupplementalPdfPreference,
	visibleRemoteReleases,
	visibleRemoteTitles,
} from './selection';

function remoteTitle(overrides: Partial<RemoteTitle> = {}): RemoteTitle {
	return {
		providerId: 'audible',
		titleId: 'B000000001',
		title: 'Example Book',
		authors: ['Example Author'],
		narrators: ['Example Narrator'],
		durationSeconds: 3600,
		coverUrl: undefined,
		supplementalPdfAvailable: false,
		acquired: false,
		availability: {
			status: 'available',
			acquirable: true,
			label: 'Available',
			detail: undefined,
		},
		unsupportedReasons: [],
		...overrides,
	};
}

function release(overrides: Partial<RemoteRelease> = {}): RemoteRelease {
	return {
		providerId: 'indexer',
		guid: 'release-1',
		indexerId: 1,
		title: 'Example Release',
		indexer: 'Example',
		sizeBytes: 100,
		protocol: 'torrent',
		seeders: 0,
		categories: [{ id: 3030, name: 'Audio/Audiobook' }],
		...overrides,
	};
}

describe('remote source selection policy', () => {
	it('filters visible titles by title, author, and narrator text after facet filters', () => {
		const unavailablePdfTitle = remoteTitle({
			titleId: 'B000000002',
			title: 'Companion Guide',
			authors: ['PDF Author'],
			narrators: ['Guide Narrator'],
			supplementalPdfAvailable: true,
			availability: {
				status: 'catalogOnly',
				acquirable: false,
				label: 'Audible catalog title',
				detail: 'Audible reports this title is not downloadable for this account.',
			},
			unsupportedReasons: ['protectedUnsupported'],
		});
		const availablePdfTitle = remoteTitle({
			titleId: 'B000000003',
			title: 'Workbook',
			authors: ['Searchable Author'],
			narrators: ['Workbook Narrator'],
			supplementalPdfAvailable: true,
		});
		const standardTitle = remoteTitle({
			titleId: 'B000000004',
			title: 'Standard Book',
			authors: ['General Author'],
			narrators: ['Searchable Narrator'],
		});

		expect(
			visibleRemoteTitles([unavailablePdfTitle, availablePdfTitle, standardTitle], {
				titleFilter: 'searchable',
				showSupplementalPdfOnly: true,
				hideUnavailableTitles: true,
			}).map((title) => title.titleId),
		).toEqual(['B000000003']);
	});

	it('toggles only acquirable titles in a copied selection set', () => {
		const selected = new Set(['B000000001']);
		const selectedTitle = remoteTitle({ titleId: 'B000000001' });
		const unavailableTitle = remoteTitle({
			titleId: 'B000000002',
			availability: {
				status: 'providerUnavailable',
				acquirable: false,
				label: 'Unavailable from Audible',
				detail: undefined,
			},
			unsupportedReasons: ['protectedUnsupported'],
		});

		const afterDeselect = toggledRemoteTitleSelection(selected, selectedTitle);
		const afterUnavailable = toggledRemoteTitleSelection(afterDeselect, unavailableTitle);
		const afterSelect = toggledRemoteTitleSelection(
			afterUnavailable,
			remoteTitle({ titleId: 'B000000003' }),
		);

		expect([...selected]).toEqual(['B000000001']);
		expect([...afterDeselect]).toEqual([]);
		expect([...afterUnavailable]).toEqual([]);
		expect([...afterSelect]).toEqual(['B000000003']);
	});

	it('toggles Supplemental PDF preference in a copied record', () => {
		const preference = { B000000001: true };

		const next = toggledSupplementalPdfPreference(preference, 'B000000001');
		const enabledByDefault = toggledSupplementalPdfPreference(next, 'B000000002');

		expect(preference).toEqual({ B000000001: true });
		expect(next).toEqual({ B000000001: false });
		expect(enabledByDefault).toEqual({ B000000001: false, B000000002: true });
	});

	it('summarizes selected titles hidden by current filters', () => {
		const visibleTitles = [
			remoteTitle({ titleId: 'B000000001' }),
			remoteTitle({ titleId: 'B000000002' }),
		];

		expect(selectedRemoteTitleSummaryText(new Set(), visibleTitles)).toBe('0 selected');
		expect(selectedRemoteTitleSummaryText(new Set(['B000000001']), visibleTitles)).toBe(
			'1 title selected',
		);
		expect(
			selectedRemoteTitleSummaryText(new Set(['B000000001', 'B000000003']), visibleTitles),
		).toBe('2 titles selected (1 title hidden by filter)');
	});

	it('filters visible releases client-side without fabricating remote titles', () => {
		const releases: RemoteRelease[] = [
			{
				providerId: 'indexer',
				guid: 'a',
				indexerId: 1,
				title: 'The Way of Kings',
				indexer: 'Example',
				sizeBytes: 100,
				protocol: 'torrent',
				seeders: 10,
				categories: [{ id: 3030, name: 'Audio/Audiobook' }],
			},
			{
				providerId: 'indexer',
				guid: 'b',
				indexerId: 2,
				title: 'Mistborn',
				indexer: 'Other',
				sizeBytes: 200,
				protocol: 'usenet',
				seeders: undefined,
				categories: [{ id: 3000, name: 'Audio' }],
			},
		];

		expect(
			visibleRemoteReleases(releases, { releaseFilter: 'way' }).map((item) => item.guid),
		).toEqual(['a']);
		expect(
			visibleRemoteReleases(releases, { releaseFilter: 'usenet' }).map((item) => item.guid),
		).toEqual(['b']);
		expect(
			visibleRemoteReleases(releases, { releaseFilter: 'nzb' }).map((item) => item.guid),
		).toEqual(['b']);
		expect(
			visibleRemoteReleases(releases, { releaseFilter: 'audiobook' }).map((item) => item.guid),
		).toEqual(['a']);
	});

	it('orders visible releases by seeders descending and keeps that order under filter', () => {
		const releases: RemoteRelease[] = [
			release({ guid: 'tpb-25', title: 'Starsight 25', indexer: 'The Pirate Bay', seeders: 25 }),
			release({ guid: 'tpb-4', title: 'Starsight 4', indexer: 'The Pirate Bay', seeders: 4 }),
			release({
				guid: 'nzbgeek',
				title: 'Starsight NZB',
				indexer: 'NZBGeek',
				protocol: 'usenet',
				seeders: undefined,
			}),
			release({ guid: 'mam-72', title: 'Starsight 72', indexer: 'MyAnonamouse', seeders: 72 }),
		];

		expect(visibleRemoteReleases(releases, { releaseFilter: '' }).map((item) => item.guid)).toEqual(
			['mam-72', 'tpb-25', 'tpb-4', 'nzbgeek'],
		);
		expect(releases.map((item) => item.guid)).toEqual(['tpb-25', 'tpb-4', 'nzbgeek', 'mam-72']);
		expect(
			visibleRemoteReleases(releases, { releaseFilter: 'pirate' }).map((item) => item.guid),
		).toEqual(['tpb-25', 'tpb-4']);
	});
});
