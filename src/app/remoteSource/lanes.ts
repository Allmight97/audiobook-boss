import type { ProviderId } from '../../lib/generated/tauri';
import type { RemoteSourceView } from './types';

export type RemoteLaneControls = {
	readonly logout: boolean;
	readonly connect: boolean;
	readonly library: boolean;
	readonly search: boolean;
	readonly setupHint: boolean;
};

export type RemoteLane = {
	readonly id: ProviderId;
	readonly label: string;
	readonly selected: boolean;
	/** Only the selected lane has an account state; the others read as not connected. */
	readonly connected: boolean;
	readonly accountMessage: string | null;
	readonly controls: RemoteLaneControls;
};

const NO_CONTROLS: RemoteLaneControls = {
	logout: false,
	connect: false,
	library: false,
	search: false,
	setupHint: false,
};

function controlsFor(id: ProviderId, connected: boolean): RemoteLaneControls {
	return id === 'audible'
		? { ...NO_CONTROLS, logout: connected, library: connected, connect: !connected }
		: { ...NO_CONTROLS, search: connected, setupHint: !connected };
}

export function remoteLanes(
	view: Pick<RemoteSourceView, 'providers' | 'providerId' | 'accountState'>,
): readonly RemoteLane[] {
	return view.providers.map((provider) => {
		const selected = provider.providerId === view.providerId;
		const connected = selected && view.accountState?.status === 'connected';
		return {
			id: provider.providerId,
			label: provider.label,
			selected,
			connected,
			accountMessage: selected ? (view.accountState?.message ?? null) : null,
			controls: selected ? controlsFor(provider.providerId, connected) : NO_CONTROLS,
		};
	});
}
