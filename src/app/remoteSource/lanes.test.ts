import { expect, it } from 'vitest';
import { remoteLanes } from './lanes';
import type {
	RemoteAccountStatus,
	RemoteSourceProviderCapabilities,
} from '../../lib/generated/tauri';

const providers: RemoteSourceProviderCapabilities[] = [
	{ providerId: 'audible', label: 'Audible' },
	{ providerId: 'indexer', label: 'Indexer' },
];

function lanesFor(providerId: 'audible' | 'indexer', status: RemoteAccountStatus | null) {
	return remoteLanes({
		providers,
		providerId,
		accountState: status ? { providerId, status, message: 'note' } : null,
	});
}

it('lists every provider with its id and label, and marks the shown one', () => {
	const lanes = lanesFor('indexer', null);

	expect(lanes.map((lane) => [lane.id, lane.label, lane.selected])).toEqual([
		['audible', 'Audible', false],
		['indexer', 'Indexer', true],
	]);
});

it('shows the Audible library and logout only once the account is connected', () => {
	const connected = lanesFor('audible', 'connected')[0];
	expect(connected.connected).toBe(true);
	expect(connected.controls).toMatchObject({ logout: true, library: true, connect: false });

	const signedOut = lanesFor('audible', 'needsAuth')[0];
	expect(signedOut.connected).toBe(false);
	expect(signedOut.controls).toMatchObject({ logout: false, library: false, connect: true });
});

it('shows the Indexer search once connected, and a setup hint with the account message otherwise', () => {
	expect(lanesFor('indexer', 'connected')[1].controls).toMatchObject({
		search: true,
		setupHint: false,
	});

	const unset = lanesFor('indexer', 'needsAuth')[1];
	expect(unset.controls).toMatchObject({ search: false, setupHint: true });
	expect(unset.accountMessage).toBe('note');
});

it('shows no controls on a lane that is not shown', () => {
	const hidden = lanesFor('audible', 'connected')[1];

	expect(hidden.connected).toBe(false);
	expect(Object.values(hidden.controls)).toEqual([false, false, false, false, false]);
});
