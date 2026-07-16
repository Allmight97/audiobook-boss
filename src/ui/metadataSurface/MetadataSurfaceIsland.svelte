<script lang="ts">
	import { onMount } from 'svelte';
	import { PopoverController } from '../../lib/ui/popover.svelte';
	import { coverArtBytesToDataUrl } from '../coverArt';
	import {
		getCurrentFileList,
		getSelectedFileIndex,
		getSelectedFiles,
		readInspectorFacts,
	} from '../fileList';
	import { readMetadataFormViewSnapshot } from '../metadataForm';
	import { getMetadataForFile } from '../metadataSession';
	import { editSurfaceState } from './editSurface.svelte';
	import MetadataSurfacePanes from './MetadataSurfacePanes.svelte';
	type Presentation = {
		open: (anchor: HTMLElement) => void;
		closeWithoutStaging: (options?: { restoreFocus?: boolean }) => void;
		isOpen: () => boolean;
	};

	interface Props {
	onDismiss?: (options?: { restoreFocus?: boolean }) => Promise<boolean>;
		onPresentationReady?: (presentation: Presentation | null) => void;
	}

	let { onDismiss = async () => true, onPresentationReady }: Props = $props();

	let anchor = $state<HTMLElement | null>(null);
	let panel = $state<HTMLElement | null>(null);
	let host = $state<HTMLElement | null>(null);
	let container = $state<HTMLElement | null>(null);
	let openVersion = $state(0);
	let dismissing = false;
	const popover = new PopoverController();
	let popoverPresentation = $state<Presentation | null>(null);

	const metadataFormSnapshot = $derived(readMetadataFormViewSnapshot());
	const activeFile = $derived(getCurrentFileList()?.files[getSelectedFileIndex()] ?? null);
	const facts = $derived(readInspectorFacts());
	const activeMetadata = $derived(activeFile ? getMetadataForFile(activeFile.path) : undefined);
	const activeTitle = $derived(
		activeMetadata?.title || facts.find((fact) => fact.label === 'File')?.value || 'Metadata',
	);
	const coverDataUrl = $derived(
		activeMetadata?.cover_art?.length ? coverArtBytesToDataUrl(activeMetadata.cover_art) : null,
	);

	$effect(() => {
		popover.setElements({ anchor, container, panel });
	});

	function open(nextAnchor: HTMLElement): void {
		anchor = nextAnchor;
		openVersion += 1;
		popover.open();
	}

	function closeWithoutStaging(options?: { restoreFocus?: boolean }): void {
		popover.close(options);
	}

	async function requestDismissal(options?: { restoreFocus?: boolean }): Promise<void> {
		if (dismissing || !popover.isOpen) return;
		dismissing = true;
		try {
			await onDismiss(options);
		} finally {
			dismissing = false;
		}
	}


	const railPresentation: Presentation = {
		open: () => {},
		closeWithoutStaging: () => {},
		isOpen: () =>
			editSurfaceState.preference === 'rail' && getSelectedFiles().length > 0,
	};

	$effect(() => {
		if (!onPresentationReady || !popoverPresentation) return;
		if (editSurfaceState.preference === 'rail') {
			if (popover.isOpen) popover.close({ restoreFocus: false });
			onPresentationReady(railPresentation);
			return;
		}
		onPresentationReady(popoverPresentation);
	});

	onMount(() => {
		container = host?.closest<HTMLElement>('.app-shell') ?? document.body;
		popoverPresentation = { open, closeWithoutStaging, isOpen: () => popover.isOpen };
		return () => onPresentationReady?.(null);
	});
</script>

<svelte:window
	onclick={(event) => {
		const target = event.target;
		if (
			popover.isOpen &&
			target instanceof Node &&
			!(target instanceof Element && target.closest('[data-metadata-selection-intent]')) &&
			!panel?.contains(target) &&
			!anchor?.contains(target)
		) {
			void requestDismissal({ restoreFocus: false });
		}
	}}
/>

<div class="metadata-surface-host" bind:this={host}>
	{#if popover.isOpen}
		<div
		bind:this={panel}
		class="app-popover metadata-surface"
		data-testid="metadata-surface"
		role="dialog"
		aria-label="Metadata editor"
		tabindex="-1"
		style={`left: ${popover.position.left}px; top: ${popover.position.top}px`}
		onkeydown={(event) => {
			if (event.key === 'Escape') {
				event.preventDefault();
				void requestDismissal();
			}
		}}
	>
		<header class="metadata-surface-header">
			{#if coverDataUrl}
				<img class="metadata-surface-cover" src={coverDataUrl} alt="" />
			{:else}
				<div class="metadata-surface-cover metadata-surface-cover-placeholder" aria-hidden="true"></div>
			{/if}
			<h2>
				{metadataFormSnapshot.mode === 'multi' && metadataFormSnapshot.selectionCount > 1
					? `${metadataFormSnapshot.selectionCount} files selected`
					: activeTitle}
			</h2>
			<button
				type="button"
				class="pill pill-ghost pill-xs"
				aria-label="Close metadata editor"
				onclick={() => void requestDismissal()}
			>
				✕
			</button>
		</header>

		<div class="metadata-surface-body">
			<MetadataSurfacePanes idPrefix="metadata-surface" layout="stacked" resetKey={openVersion} />
		</div>
		</div>
	{/if}
</div>

<style>
	.metadata-surface {
		width: 330px;
		max-height: calc(100% - (2 * var(--space-3)));
		overflow: auto;
	}

	.metadata-surface-header {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 12px 14px;
		border-bottom: 1px solid var(--border-primary);
	}

	.metadata-surface-cover {
		width: 34px;
		height: 34px;
		flex: 0 0 auto;
		border-radius: 6px;
		object-fit: cover;
	}

	.metadata-surface-cover-placeholder { background: var(--bg-hover); }
	.metadata-surface-header h2 {
		min-width: 0;
		flex: 1 1 auto;
		overflow: hidden;
		margin: 0;
		color: var(--text-primary);
		font-size: 13px;
		font-weight: 600;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.metadata-surface-body { padding: 12px 14px; }

</style>
