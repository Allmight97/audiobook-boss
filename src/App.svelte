<script lang="ts">
	import { onMount } from 'svelte';
	import { LeftColumnIsland } from './ui/leftColumn';
	import { MetadataManagerIsland } from './ui/metadataManager';
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
		void initializeAppSettingsControlPlane();
	});
</script>

<svelte:window onkeydown={handleGlobalKeyDown} />

<AppShellIsland>
	<div class="main-container">
		<LeftColumnIsland {readWorkActivityByInputId} />

		<div class="right-column-wrapper">
			<div class="panel right-column-panel metadata-manager-panel">
				<MetadataManagerIsland />
			</div>
		</div>
	</div>
</AppShellIsland>
<MetadataLookupIsland />
<RemoteSourceAcquireDialog />
<CollisionDialogIsland />
<AppSettingsDialogIsland />
