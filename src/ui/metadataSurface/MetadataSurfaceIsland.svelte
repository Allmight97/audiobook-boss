<script lang="ts">
	import { onMount } from 'svelte';
	import { PopoverController } from '../../lib/ui/popover.svelte';
	import {
		onClearCoverArt,
		onLoadCoverArtFromFilePicker,
		onLoadCoverArtFromInput,
	} from '../coverArt';
	import CoverArtIsland from '../coverArt/CoverArtIsland.svelte';
	import { readActiveFileChapters, readInspectorFacts } from '../fileList';
	import { saveMetadataFromUI } from '../metadataSession';
	import {
		MetadataFormFieldsIsland,
		onMetadataFormActionSelectChange,
		onMetadataFormFieldInput,
		readMetadataFormViewSnapshot,
	} from '../metadataForm';
	import { readOutputDisplaySnapshot } from '../outputPanel';

	type TabId = 'metadata' | 'facts' | 'chapters' | 'output';
	type Presentation = {
		open: (anchor: HTMLElement) => void;
		closeWithoutStaging: () => void;
	};

	interface Props {
		onDismiss?: () => Promise<boolean>;
		onPresentationReady?: (presentation: Presentation | null) => void;
	}

	let { onDismiss = async () => true, onPresentationReady }: Props = $props();

	const tabs: Array<{ id: TabId; label: string }> = [
		{ id: 'metadata', label: 'Metadata' },
		{ id: 'facts', label: 'Facts' },
		{ id: 'chapters', label: 'Chapters' },
		{ id: 'output', label: 'Output' },
	];
	let activeTab = $state<TabId>('metadata');
	let anchor = $state<HTMLElement | null>(null);
	let panel = $state<HTMLElement | null>(null);
	let host = $state<HTMLElement | null>(null);
	let container = $state<HTMLElement | null>(null);
	let dismissing = false;
	const popover = new PopoverController();

	const metadataFormSnapshot = $derived(readMetadataFormViewSnapshot());
	const facts = $derived(readInspectorFacts());
	const chapters = $derived(readActiveFileChapters());
	const output = $derived(readOutputDisplaySnapshot());

	$effect(() => {
		popover.setElements({ anchor, container, panel });
	});

	function open(nextAnchor: HTMLElement): void {
		anchor = nextAnchor;
		activeTab = 'metadata';
		popover.open();
	}

	function closeWithoutStaging(): void {
		popover.close();
	}

	async function requestDismissal(): Promise<void> {
		if (dismissing || !popover.isOpen) return;
		dismissing = true;
		try {
			await onDismiss();
		} finally {
			dismissing = false;
		}
	}

	function handleTabKeydown(event: KeyboardEvent, index: number): void {
		if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
		event.preventDefault();
		const nextIndex =
			event.key === 'Home'
				? 0
				: event.key === 'End'
					? tabs.length - 1
					: (index + (event.key === 'ArrowRight' ? 1 : tabs.length - 1)) % tabs.length;
		const next = tabs[nextIndex];
		if (!next) return;
		activeTab = next.id;
		document.getElementById(`metadata-surface-tab-${next.id}`)?.focus();
	}

	function formatChapterTime(milliseconds: number): string {
		const totalSeconds = Math.max(0, Math.floor(milliseconds / 1000));
		const hours = Math.floor(totalSeconds / 3600);
		const minutes = Math.floor((totalSeconds % 3600) / 60);
		const seconds = totalSeconds % 60;
		return hours > 0
			? `${hours}:${minutes.toString().padStart(2, '0')}:${seconds.toString().padStart(2, '0')}`
			: `${minutes}:${seconds.toString().padStart(2, '0')}`;
	}

	onMount(() => {
		container = host?.closest<HTMLElement>('.app-shell') ?? document.body;
		onPresentationReady?.({ open, closeWithoutStaging });
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
			void requestDismissal();
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

		<div class="metadata-surface-tabs" role="tablist" aria-label="Metadata editor sections">
			{#each tabs as tab, index}
				<button
					id={`metadata-surface-tab-${tab.id}`}
					type="button"
					role="tab"
					aria-selected={activeTab === tab.id}
					aria-controls={`metadata-surface-panel-${tab.id}`}
					tabindex={activeTab === tab.id ? 0 : -1}
					onclick={() => {
						activeTab = tab.id;
					}}
					onkeydown={(event) => handleTabKeydown(event, index)}
				>
					{tab.label}
				</button>
			{/each}
		</div>

		{#if activeTab === 'metadata'}
			<div id="metadata-surface-panel-metadata" role="tabpanel" aria-labelledby="metadata-surface-tab-metadata">
				<div id="metadata-form" data-multi-select={metadataFormSnapshot.mode === 'multi'} class="metadata-surface-form">
					<div class="metadata-cover-cell">
						<CoverArtIsland
							onLoadFromFile={onLoadCoverArtFromFilePicker}
							onLoadFromInput={onLoadCoverArtFromInput}
							onClearCoverArt={onClearCoverArt}
						/>
					</div>
					<div class="metadata-fields-cell">
						<MetadataFormFieldsIsland
							onFieldInput={onMetadataFormFieldInput}
							onActionChange={onMetadataFormActionSelectChange}
							onSaveMetadata={() => void saveMetadataFromUI()}
						/>
					</div>
				</div>
			</div>
		{:else if activeTab === 'facts'}
			<div id="metadata-surface-panel-facts" role="tabpanel" aria-labelledby="metadata-surface-tab-facts" class="metadata-surface-readout">
				{#each facts as fact}
					<div><span>{fact.label}</span><strong title={fact.title}>{fact.value}</strong></div>
				{/each}
			</div>
		{:else if activeTab === 'chapters'}
			<div id="metadata-surface-panel-chapters" role="tabpanel" aria-labelledby="metadata-surface-tab-chapters" class="metadata-surface-readout">
				{#if chapters.length === 0}
					<p>No embedded chapters on the active file.</p>
				{:else}
					<ol class="metadata-surface-chapters">
						{#each chapters as chapter, index}
							<li><strong>{chapter.title?.trim() || `Chapter ${index + 1}`}</strong><span>{formatChapterTime(chapter.startMs)} – {formatChapterTime(chapter.endMs)}</span></li>
						{/each}
					</ol>
				{/if}
			</div>
		{:else}
			<div id="metadata-surface-panel-output" role="tabpanel" aria-labelledby="metadata-surface-tab-output" class="metadata-surface-readout">
				<span>Planned output</span>
				<strong title={output.previewTitle}>{output.previewText}</strong>
			</div>
		{/if}
		</div>
	{/if}
</div>

<style>
	.metadata-surface {
		width: min(52rem, calc(100% - (2 * var(--space-3))));
		max-height: calc(100% - (2 * var(--space-3)));
		overflow: auto;
	}

	.metadata-surface-header,
	.metadata-surface-tabs {
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

	.metadata-surface-tabs { gap: var(--space-1); margin: 0 calc(-1 * var(--space-1)) var(--space-3); border-bottom: 1px solid var(--border-primary); }
	.metadata-surface-tabs button { margin-top: 0; padding: var(--space-2) var(--space-3); border-radius: var(--radius-sm) var(--radius-sm) 0 0; background: transparent; color: var(--text-muted); font-size: var(--text-sm); }
	.metadata-surface-tabs button[aria-selected='true'] { box-shadow: inset 0 -2px 0 var(--accent-primary); color: var(--text-primary); }

	.metadata-surface-form {
		display: grid;
		grid-template-columns: minmax(10rem, 0.8fr) minmax(0, 3fr);
		gap: var(--space-3);
		align-items: start;
	}

	.metadata-cover-cell,
	.metadata-fields-cell {
		min-width: 0;
	}

	.metadata-cover-cell {
		display: flex;
		justify-content: center;
	}

	.metadata-surface-readout { display: grid; gap: var(--space-2); color: var(--text-secondary); font-size: var(--text-md); }
	.metadata-surface-readout > div { display: grid; grid-template-columns: 8rem minmax(0, 1fr); gap: var(--space-2); }
	.metadata-surface-readout span { color: var(--text-muted); }
	.metadata-surface-readout strong { overflow: hidden; color: var(--text-primary); text-overflow: ellipsis; white-space: nowrap; }
	.metadata-surface-readout p { margin: 0; color: var(--text-muted); }
	.metadata-surface-chapters { display: grid; gap: var(--space-2); margin: 0; padding-left: var(--space-5); }
	.metadata-surface-chapters li { display: flex; justify-content: space-between; gap: var(--space-3); }
	.metadata-surface-chapters span { white-space: nowrap; }

	@media (max-width: 900px) {
		.metadata-surface-form {
			grid-template-columns: 1fr;
		}

		.metadata-cover-cell {
			justify-content: flex-start;
		}
	}
</style>
