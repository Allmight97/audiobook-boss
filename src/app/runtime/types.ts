import type { InputCapability } from '../../lib/tauri/capabilities/input';
import type { MetadataCapability } from '../../lib/tauri/capabilities/metadata';
import type { SettingsCapability } from '../../lib/tauri/capabilities/settings';
import type { InputOwner } from '../inputSession/owner';
import type { RemoteSourceOwner } from '../remoteSource/owner';
import type { AtomRegistry } from './reactivity';

export type RuntimeCapabilities = {
	readonly input?: InputCapability;
	readonly metadata?: MetadataCapability;
	readonly settings?: SettingsCapability;
};

export type AppRuntime = {
	readonly input: InputOwner;
	readonly remoteSource: RemoteSourceOwner;
	readonly registry: AtomRegistry.AtomRegistry;
	dispose(): void;
};
