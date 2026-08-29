import { describe, expect, it } from 'vitest';

import * as remoteSource from '../../../ui/remoteSource';

const EXPECTED_REMOTE_SOURCE_EXPORTS = [
	'companionSummaryForInputIds',
	'hasSupplementalAssetsForInputId',
	'purgeRemoteSourceSessionsForInputIds',
	'registerRemoteSourceSupplementalAssets',
	'releaseRemoteSourceSessionRetainers',
	'retainRemoteSourceSessionsForInputIds',
	'supplementalAssetsByInputIdForProcessing',
] as const;

const FORBIDDEN_REMOTE_SOURCE_IMPORTS = [
	'remoteSource/sessionAssets',
	'remoteSource/workflow',
	'remoteSource/state',
	'remoteSource/RemoteSourceAcquireView',
	'app/remoteSource/sessionAssets',
	'app/remoteSource/workflow',
	'app/remoteSource/state',
] as const;

const processingSources = import.meta.glob<string>('../**/*.ts', {
	eager: true,
	import: 'default',
	query: '?raw',
});

function importViolations(source: string): string[] {
	return FORBIDDEN_REMOTE_SOURCE_IMPORTS.filter((pattern) => source.includes(pattern));
}

function productionProcessingSources(): [string, string][] {
	return Object.entries(processingSources).filter(
		([file]) => !file.includes('/__tests__/') && !/\.(test|spec)\.ts$/.test(file),
	);
}

describe('Processing remoteSource Public API Strip boundary', () => {
	it('pins the remoteSource public export strip', () => {
		expect(Object.keys(remoteSource).sort()).toEqual([...EXPECTED_REMOTE_SOURCE_EXPORTS].sort());
	});

	it('does not export private sessionAssets helpers or acquisition workflow symbols', () => {
		expect(remoteSource).not.toHaveProperty('supplementalAssetsForInputIds');
		expect(remoteSource).not.toHaveProperty('removeRemoteSourceSupplementalAssets');
		expect(remoteSource).not.toHaveProperty('purgeSuccessfulRemoteSourceSessions');
	});

	it('keeps production processing code on the remoteSource public import surface', () => {
		const violations = productionProcessingSources().flatMap(([file, source]) => {
			const matched = importViolations(source);
			return matched.map((pattern) => `${file} imports ${pattern}`);
		});

		expect(violations).toEqual([]);
	});
});
