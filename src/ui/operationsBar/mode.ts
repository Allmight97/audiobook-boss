export type OpsMode = 'collapsed' | 'open' | 'pinned';

export function toggleOpsDisclosure(mode: OpsMode): OpsMode {
	return mode === 'pinned' ? mode : mode === 'collapsed' ? 'open' : 'collapsed';
}

export function toggleOpsPin(mode: OpsMode): OpsMode {
	return mode === 'pinned' ? 'open' : 'pinned';
}
