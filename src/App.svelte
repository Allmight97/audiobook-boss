<script lang="ts">
	import { onMount } from 'svelte';
	import { FileListIsland, requestMetadataSurfaceDismissal, setMetadataSurfacePresentation } from './ui/fileList';
	import FileImportIsland from './ui/fileImport/FileImportIsland.svelte';
	import { MetadataRailIsland, MetadataSurfaceIsland } from './ui/metadataSurface';
	import { readWorkActivityByInputId } from './ui/workCenter';
	import { MetadataLookupIsland } from './ui/metadataLookup';
	import { AppShellIsland } from './ui/appShell';
	import CollisionDialogIsland from './ui/collisionDialog/CollisionDialogIsland.svelte';
	import RemoteSourceAcquireDialog from './ui/remoteSource/RemoteSourceAcquireDialog.svelte';
	import { saveMetadataFromUI } from './ui/metadataSession';
	import {
		AppSettingsDialogIsland,
		initializeAppSettingsControlPlane,
		openAppSettingsDialog,
	} from './ui/appSettings';
	import { initFrontendErrorLogBridge } from './lib/frontendLogBridge';

	function handleGlobalKeyDown(event: KeyboardEvent): void {
		if ((event.metaKey || event.ctrlKey) && event.key === 's') {
			event.preventDefault();
			void saveMetadataFromUI();
		}
		if ((event.metaKey || event.ctrlKey) && event.key === ',') {
			event.preventDefault();
			void openAppSettingsDialog();
		}
	}

	onMount(() => {
		initFrontendErrorLogBridge();
		void initializeAppSettingsControlPlane();
	});
</script>

<svelte:window onkeydown={handleGlobalKeyDown} />

<AppShellIsland>
	{#snippet rail()}
		<MetadataRailIsland />
	{/snippet}
	{#snippet overlay()}
		<MetadataSurfaceIsland
			onDismiss={requestMetadataSurfaceDismissal}
			onPresentationReady={setMetadataSurfacePresentation}
		/>
	{/snippet}
	<div class="file-area" data-testid="file-area">
		<FileImportIsland>
			{#snippet children(dropTarget)}
				<FileListIsland {...dropTarget} {readWorkActivityByInputId} />
			{/snippet}
		</FileImportIsland>
	</div>
</AppShellIsland>
<MetadataLookupIsland />
<RemoteSourceAcquireDialog />
<CollisionDialogIsland />
<AppSettingsDialogIsland />

<style>
	.file-area {
		display: flex;
		flex: 1 1 auto;
		flex-direction: column;
		min-width: 0;
		min-height: 0;
		padding: var(--density-pad);
		overflow: hidden;
	}
</style>
