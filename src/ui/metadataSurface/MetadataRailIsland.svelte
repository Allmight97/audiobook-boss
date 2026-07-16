<script lang="ts">
	import { coverArtBytesToDataUrl } from '../coverArt';
	import {
		getCurrentFileList,
		getSelectedFileIndex,
		getSelectedFiles,
		readActiveFileChapters,
		readInspectorFacts,
	} from '../fileList';
	import { getMetadataForFile } from '../metadataSession';
	import { readMetadataFormViewSnapshot } from '../metadataForm';
	import { formatDuration } from '../../types/audio';
	import MetadataSurfacePanes from './MetadataSurfacePanes.svelte';

	const metadataFormSnapshot = $derived(readMetadataFormViewSnapshot());
	const activeFile = $derived(getCurrentFileList()?.files[getSelectedFileIndex()] ?? null);
	const selectedFiles = $derived(getSelectedFiles());
	const facts = $derived(readInspectorFacts());
	const chapters = $derived(readActiveFileChapters());
	const activeMetadata = $derived(activeFile ? getMetadataForFile(activeFile.path) : undefined);
	const activeTitle = $derived(
		activeMetadata?.title || facts.find((fact) => fact.label === 'File')?.value || 'Metadata',
	);
	const activeAuthor = $derived(activeMetadata?.artist || '—');
	const activeDuration = $derived(formatDuration(activeFile?.duration));
	const coverDataUrl = $derived(
		activeMetadata?.cover_art?.length ? coverArtBytesToDataUrl(activeMetadata.cover_art) : null,
	);
	const hasSelection = $derived(selectedFiles.length > 0 && activeFile !== null);
</script>

<aside class="metadata-rail" data-testid="metadata-rail" aria-label="Metadata editor">
	{#if !hasSelection}
		<div class="metadata-rail-empty">Select a book to edit its details.</div>
	{:else}
		<header class="metadata-rail-head">
			{#if coverDataUrl}
				<img class="metadata-rail-cover" src={coverDataUrl} alt="" />
			{:else}
				<div class="metadata-rail-cover metadata-rail-cover-placeholder" aria-hidden="true"></div>
			{/if}
			<div class="metadata-rail-title-group">
				{#if metadataFormSnapshot.mode === 'multi' && metadataFormSnapshot.selectionCount > 1}
					<h2>{metadataFormSnapshot.selectionCount} files selected</h2>
				{:else}
					<h2>{activeTitle}</h2>
					<p>{activeAuthor} · {activeDuration} · {chapters.length} chapters</p>
				{/if}
			</div>
		</header>
		<div class="metadata-rail-pane">
			<MetadataSurfacePanes idPrefix="metadata-rail" layout="stacked" />
		</div>
	{/if}
</aside>

<style>
	.metadata-rail {
		display: flex;
		min-width: 0;
		flex: 1 1 auto;
		flex-direction: column;
		overflow: auto;
	}

	.metadata-rail-empty {
		display: grid;
		flex: 1 1 auto;
		place-items: center;
		padding: var(--space-5);
		color: var(--text-muted);
		font-size: var(--text-sm);
		text-align: center;
	}

	.metadata-rail-head {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		padding: var(--space-3) var(--space-4) 0;
	}

	.metadata-rail-cover {
		width: 56px;
		height: 56px;
		flex: 0 0 auto;
		border-radius: var(--radius-md);
		object-fit: cover;
	}

	.metadata-rail-cover-placeholder { background: var(--bg-hover); }
	.metadata-rail-title-group { min-width: 0; }
	.metadata-rail-title-group h2,
	.metadata-rail-title-group p { margin: 0; }
	.metadata-rail-title-group h2 { overflow: hidden; color: var(--text-primary); font-size: 15px; font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
	.metadata-rail-title-group p { overflow: hidden; margin-top: 2px; color: var(--text-muted); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
	.metadata-rail-pane { padding: 0 var(--space-4) var(--space-4); }
</style>
