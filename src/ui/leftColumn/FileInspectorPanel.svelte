<script lang="ts">
	import { readCombinedSizeText } from '../fileList';
	import { fileListAtomRegistry, fileListSessionAtom } from '../fileList/state';
	import { getInspectorState, inspectorAtom } from '../fileList/inspectorState';

	const fileListSessionStore = {
		subscribe(run: (value: unknown) => void) {
			return fileListAtomRegistry.subscribe(fileListSessionAtom, run, { immediate: true });
		},
	};
	const inspectorStore = {
		subscribe(run: (value: unknown) => void) {
			return fileListAtomRegistry.subscribe(inspectorAtom, run, { immediate: true });
		},
	};

	const session = $fileListSessionStore;
	const inspectorSnapshot = $inspectorStore;
	const inspectorState = $derived.by(() => {
		inspectorSnapshot;
		return getInspectorState();
	});
	const combinedSizeText = $derived.by(() => {
		session;
		return readCombinedSizeText();
	});
</script>

<section
	class="left-column-panel file-inspector-panel section-divider file-properties-pinned inspector-footer"
	aria-label="Selected File Properties"
	data-testid="file-inspector-panel"
>
	<div class="inspector-header">
		<span class="inspector-context" aria-live="polite">
			{#if inspectorState.contextVariant === 'empty'}
				<span class="context-empty">{inspectorState.contextText}</span>
			{:else}
				<span class="context-filename" title={inspectorState.contextText}>
					{inspectorState.contextText}
				</span>
				{#if inspectorState.contextDetail}
					<span class="context-position">{inspectorState.contextDetail}</span>
				{/if}
			{/if}
		</span>
	</div>

	<div class="grid grid-cols-2 gap-x-3 gap-y-0.5 text-xs">
		<span class="property-label">Bitrate:</span><span class="property-value">{inspectorState.bitrateText}</span>
		<span class="property-label">Sample Rate:</span><span class="property-value">{inspectorState.sampleRateText}</span>
		<span class="property-label">Channels:</span><span class="property-value">{inspectorState.channelsText}</span>
		<span class="property-label">Codec:</span><span class="property-value">{inspectorState.codecText}</span>
		<span class="property-label">Decoder:</span><span class="property-value">{inspectorState.decoderText}</span>
		<span class="property-label">File Size:</span><span class="property-value">{inspectorState.fileSizeText}</span>
		<span class="property-label">Supplemental:</span><span
			class="property-value"
			title={inspectorState.companionsTitle}>{inspectorState.companionsText}</span
		>
		<span class="property-label">Combined Size:</span><span class="property-value"
			>{combinedSizeText}</span
		>
	</div>
</section>
