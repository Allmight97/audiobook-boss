export const remoteSourceAcquireState = $state({
	isOpen: false,
});

export function openRemoteSourceAcquire(): void {
	remoteSourceAcquireState.isOpen = true;
}

export function closeRemoteSourceAcquire(): void {
	remoteSourceAcquireState.isOpen = false;
}
