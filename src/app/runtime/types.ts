import type { EngineCapability } from '../../lib/tauri/capabilities/engine';
import type { InputCapability } from '../../lib/tauri/capabilities/input';
import type { MetadataCapability } from '../../lib/tauri/capabilities/metadata';
import type { SettingsOwner } from '../appSettings';
import type { InputOwner } from '../inputSession';
import type { MetadataLookupOwner } from '../metadataLookup';
import type { MetadataOwner } from '../metadataSession';
import type { EncodingOwner } from '../encoding';
import type { OutputPlanOwner } from '../outputPlan';
import type { ProcessingOwner } from '../processing';
import type { RemoteSourceOwner, RemoteSourceOwnerDeps } from '../remoteSource';
import type { WorkOperationsOwner } from '../workOperations';
import type { EngineLink } from '../engineLink';

export type RuntimeCapabilities = {
	readonly engine?: EngineCapability;
	readonly input?: InputCapability;
	readonly metadata?: MetadataCapability;
	readonly remoteSource?: Omit<RemoteSourceOwnerDeps, 'link'>;
};

/** The connection's own state: whether it is attached, and refused posts. */
export type EngineStatus = Pick<EngineLink, 'attachment' | 'refusal' | 'dismissRefusal'>;

export type AppRuntime = {
	readonly engine: EngineStatus;
	readonly input: InputOwner;
	readonly metadata: MetadataOwner;
	readonly lookup: MetadataLookupOwner;
	readonly encoding: EncodingOwner;
	readonly output: OutputPlanOwner;
	readonly remoteSource: RemoteSourceOwner;
	readonly settings: SettingsOwner;
	readonly processing: ProcessingOwner;
	readonly workOperations: WorkOperationsOwner;
	initialize(): Promise<void>;
	dispose(): void;
};
