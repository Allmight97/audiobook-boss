<script lang="ts">
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

	interface Props {
		idPrefix: string;
		layout?: 'grid' | 'stacked';
		resetKey?: number;
	}

	let { idPrefix, layout = 'grid', resetKey = 0 }: Props = $props();

	const tabs: Array<{ id: TabId; label: string }> = [
		{ id: 'metadata', label: 'Metadata' },
		{ id: 'facts', label: 'Facts' },
		{ id: 'chapters', label: 'Chapters' },
		{ id: 'output', label: 'Output' },
	];
	let activeTab = $state<TabId>('metadata');
	let lastResetKey = $state<number | undefined>(undefined);
	const metadataFormSnapshot = $derived(readMetadataFormViewSnapshot());
	const facts = $derived(readInspectorFacts());
	const chapters = $derived(readActiveFileChapters());
	const output = $derived(readOutputDisplaySnapshot());

	$effect(() => {
		if (lastResetKey === undefined) {
			lastResetKey = resetKey;
			return;
		}
		if (resetKey === lastResetKey) return;
		lastResetKey = resetKey;
		activeTab = 'metadata';
	});

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
		document.getElementById(`${idPrefix}-tab-${next.id}`)?.focus();
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
</script>

<div class="tab-strip metadata-surface-tabs" role="tablist" aria-label="Metadata editor sections">
	{#each tabs as tab, index}
		<button
			id={`${idPrefix}-tab-${tab.id}`}
			type="button"
			class="tab"
			class:on={activeTab === tab.id}
			role="tab"
			aria-selected={activeTab === tab.id}
			aria-controls={`${idPrefix}-panel-${tab.id}`}
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
	<div id={`${idPrefix}-panel-metadata`} role="tabpanel" aria-labelledby={`${idPrefix}-tab-metadata`}>
		<div
			id="metadata-form"
			data-multi-select={metadataFormSnapshot.mode === 'multi'}
			class="metadata-surface-form"
			class:metadata-surface-form--stacked={layout === 'stacked'}
		>
			<div class="metadata-cover-cell">
				<CoverArtIsland
					onLoadFromFile={onLoadCoverArtFromFilePicker}
					onLoadFromInput={onLoadCoverArtFromInput}
					onClearCoverArt={onClearCoverArt}
				/>
			</div>
			<div class="metadata-fields-cell">
				<MetadataFormFieldsIsland
					{layout}
					onFieldInput={onMetadataFormFieldInput}
					onActionChange={onMetadataFormActionSelectChange}
					onSaveMetadata={() => void saveMetadataFromUI()}
				/>
			</div>
		</div>
	</div>
{:else if activeTab === 'facts'}
	<div id={`${idPrefix}-panel-facts`} role="tabpanel" aria-labelledby={`${idPrefix}-tab-facts`} class="metadata-surface-readout">
		{#each facts as fact}
			<div><span>{fact.label}</span><strong title={fact.title}>{fact.value}</strong></div>
		{/each}
	</div>
{:else if activeTab === 'chapters'}
	<div id={`${idPrefix}-panel-chapters`} role="tabpanel" aria-labelledby={`${idPrefix}-tab-chapters`} class="metadata-surface-readout">
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
	<div id={`${idPrefix}-panel-output`} role="tabpanel" aria-labelledby={`${idPrefix}-tab-output`} class="metadata-surface-readout">
		<span>Planned output</span>
		<strong title={output.previewTitle}>{output.previewText}</strong>
	</div>
{/if}

<style>
	.metadata-surface-tabs {
		gap: var(--space-1);
		margin: 0 calc(-1 * var(--space-1)) var(--space-3);
		border-bottom: 1px solid var(--border-primary);
	}

	.metadata-surface-tabs .tab {
		margin-top: 0;
		padding: var(--space-2) var(--space-3);
		border-radius: var(--radius-sm) var(--radius-sm) 0 0;
		background: transparent;
		color: var(--text-muted);
		font-size: var(--text-sm);
	}

	.metadata-surface-tabs .tab.on {
		box-shadow: inset 0 -2px 0 var(--accent-primary);
		color: var(--text-primary);
	}

	.metadata-surface-form {
		display: grid;
		grid-template-columns: minmax(10rem, 0.8fr) minmax(0, 3fr);
		gap: var(--space-3);
		align-items: start;
	}

	.metadata-surface-form--stacked {
		grid-template-columns: minmax(0, 1fr);
	}

	.metadata-surface-form--stacked .metadata-fields-cell { order: 1; }
	.metadata-surface-form--stacked .metadata-cover-cell { order: 2; justify-content: flex-start; }

	.metadata-cover-cell,
	.metadata-fields-cell { min-width: 0; }

	.metadata-cover-cell { display: flex; justify-content: center; }

	.metadata-surface-readout { display: grid; gap: var(--space-2); color: var(--text-secondary); font-size: var(--text-md); }
	.metadata-surface-readout > div { display: grid; grid-template-columns: 8rem minmax(0, 1fr); gap: var(--space-2); }
	.metadata-surface-readout span { color: var(--text-muted); }
	.metadata-surface-readout strong { overflow: hidden; color: var(--text-primary); text-overflow: ellipsis; white-space: nowrap; }
	.metadata-surface-readout p { margin: 0; color: var(--text-muted); }
	.metadata-surface-chapters { display: grid; gap: var(--space-2); margin: 0; padding-left: var(--space-5); }
	.metadata-surface-chapters li { display: flex; justify-content: space-between; gap: var(--space-3); }
	.metadata-surface-chapters span { white-space: nowrap; }

	@media (max-width: 900px) {
		.metadata-surface-form { grid-template-columns: 1fr; }
		.metadata-cover-cell { justify-content: flex-start; }
	}
</style>
