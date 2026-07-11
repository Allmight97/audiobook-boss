import { describe, expect, it } from 'vitest';

import * as remoteSource from '../../remoteSource';

const EXPECTED_REMOTE_SOURCE_EXPORTS = [
	'companionSummaryForInputIds',
	'hasSupplementalAssetsForInputId',
	'openRemoteSourceAcquire',
	'purgeRemoteSourceSessionsForInputIds',
	'registerRemoteSourceSupplementalAssets',
	'releaseRemoteSourceSessionRetainers',
	'retainRemoteSourceSessionsForInputIds',
	'supplementalAssetsByInputIdForProcessing',
] as const;

const FORBIDDEN_REMOTE_SOURCE_IMPORTS = [
	'remoteSource/sessionAssets.svelte',
	'remoteSource/acquisitionWorkflow',
	'remoteSource/acquisitionState.svelte',
	'remoteSource/state.svelte',
	'remoteSource/RemoteSourceAcquireDialog.svelte',
	'remoteSource/acquisitionAccount',
	'remoteSource/remoteSourceSelection',
] as const;

const statusPanelSources = import.meta.glob<string>('../**/*.{ts,svelte}', {
	eager: true,
	import: 'default',
	query: '?raw',
});

function importViolations(source: string): string[] {
	return FORBIDDEN_REMOTE_SOURCE_IMPORTS.filter((pattern) => source.includes(pattern));
}

function productionStatusPanelSources(): [string, string][] {
	return Object.entries(statusPanelSources).filter(
		([file]) => !file.includes('/__tests__/') && !/\.(test|spec)\.ts$/.test(file),
	);
}

describe('Status Panel remoteSource Public API Strip boundary', () => {
	it('pins the remoteSource public export strip', () => {
		expect(Object.keys(remoteSource).sort()).toEqual([...EXPECTED_REMOTE_SOURCE_EXPORTS].sort());
	});

	it('does not export private sessionAssets helpers or acquisition workflow symbols', () => {
		expect(remoteSource).not.toHaveProperty('supplementalAssetsForInputIds');
		expect(remoteSource).not.toHaveProperty('removeRemoteSourceSupplementalAssets');
		expect(remoteSource).not.toHaveProperty('purgeSuccessfulRemoteSourceSessions');
	});

	it('keeps production statusPanel code on the remoteSource public import surface', () => {
		const violations = productionStatusPanelSources().flatMap(([file, source]) => {
			const matched = importViolations(source);
			return matched.map((pattern) => `${file} imports ${pattern}`);
		});

		expect(violations).toEqual([]);
	});
});
