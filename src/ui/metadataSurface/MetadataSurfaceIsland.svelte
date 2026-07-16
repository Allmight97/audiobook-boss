<script lang="ts">
	import { onMount } from 'svelte';
	import { PopoverController } from '../../lib/ui/popover.svelte';
	import { getSelectedFiles } from '../fileList';
	import { readMetadataFormViewSnapshot } from '../metadataForm';
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
			<div>
				<h2>Metadata</h2>
				{#if metadataFormSnapshot.mode === 'multi' && metadataFormSnapshot.selectionCount > 1}
					<p>{metadataFormSnapshot.selectionCount} files selected</p>
				{/if}
			</div>
			<button
				type="button"
				class="metadata-surface-close"
				aria-label="Close metadata editor"
				onclick={() => void requestDismissal()}
			>
				×
			</button>
		</header>

		<MetadataSurfacePanes idPrefix="metadata-surface" resetKey={openVersion} />
		</div>
	{/if}
</div>

<style>
	.metadata-surface {
		width: min(52rem, calc(100% - (2 * var(--space-3))));
		max-height: calc(100% - (2 * var(--space-3)));
		overflow: auto;
	}

	.metadata-surface-header {
		display: flex;
		align-items: center;
	}

	.metadata-surface-header {
		justify-content: space-between;
		gap: var(--space-3);
		margin-bottom: var(--space-2);
	}

	.metadata-surface-header h2,
	.metadata-surface-header p { margin: 0; }
	.metadata-surface-header h2 { color: var(--text-primary); font-size: var(--text-lg); }
	.metadata-surface-header p { color: var(--text-muted); font-size: var(--text-sm); }
	.metadata-surface-close { margin-top: 0; padding: var(--space-1) var(--space-2); background: transparent; color: var(--text-muted); }
	.metadata-surface-close:hover, .metadata-surface-close:focus-visible { background: var(--bg-hover); color: var(--text-primary); }

</style>
