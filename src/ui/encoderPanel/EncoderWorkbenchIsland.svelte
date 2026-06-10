<script lang="ts">
	import { onMount } from 'svelte';
	import { readEstimatedSizeText } from '../outputPanel';
	import {
		handleBitrateModeChange,
		handleBitrateValueChange,
		handleChannelsSelectionChange,
		handleFlavorChange,
		handleFdkAfterburnerChange,
		handleNativeTwoloopChange,
		handleQualityValueChange,
		handleSampleRateSelectionChange,
		initializeEncoderPanelLogic,
	} from './logic';
	import { encoderPanelState } from './state.svelte';
	import {
		bitrateModeLabel,
		channelLabel,
		channelsDetailText,
		encoderLabel,
		encoderOptionDisabled,
		qualityLabel,
		sampleRateDetailText,
		sampleRateLabel,
	} from './view';

	const estimatedSizeText = $derived(readEstimatedSizeText());

	onMount(() => {
		initializeEncoderPanelLogic();
	});
</script>

<div id="encoder-settings-panel" class="encoder-workbench-panel" data-testid="encoder-settings-panel">
	<div class="encoder-workbench-header">
		<h3>Encoder</h3>
		<span class="inline-info">
			(<span id="estimated-size" data-testid="estimated-size">{estimatedSizeText}</span>)
		</span>
	</div>

	<div class="encoder-workbench-grid">
		<div class="encoder-field-row">
			<label for="adv-encoder">Encoder</label>
			<div class="encoder-field-stack">
				<select
					id="adv-encoder"
					data-testid="encoder-select"
					value={encoderPanelState.flavor}
					disabled={encoderPanelState.encoderOptions.length === 0}
					onchange={handleFlavorChange}
				>
					{#if encoderPanelState.encoderOptions.length === 0}
						<option value="auto">Loading…</option>
					{:else}
						{#each encoderPanelState.encoderOptions as encoderOption}
							<option value={encoderOption} disabled={encoderOptionDisabled(encoderOption)}>
								{encoderLabel(encoderOption)}
							</option>
						{/each}
					{/if}
				</select>
				<p
					id="encoder-availability-hint"
					class="field-hint"
					data-testid="encoder-availability-hint"
				>
					{encoderPanelState.availabilityHint}
				</p>
			</div>
		</div>

		<div class="encoder-field-row">
			<span class="label">Profile</span>
			<div class="profile-display profile-display-workbench" data-testid="profile-display">
				<span id="encoder-profile-display">{encoderPanelState.profileDisplay}</span>
				<span class="readonly-badge">read-only</span>
			</div>
		</div>

		<div class="encoder-field-row">
			<label for="adv-bitrate-mode">Bitrate Mode</label>
			<select
				id="adv-bitrate-mode"
				data-testid="bitrate-mode-select"
				value={encoderPanelState.bitrateModeSelection}
				disabled={encoderPanelState.bitrateModeOptions.length === 0}
				onchange={handleBitrateModeChange}
			>
				{#each encoderPanelState.bitrateModeOptions as bitrateModeOption}
					<option
						value={bitrateModeOption}
						disabled={!encoderPanelState.bitrateModeAvailability[bitrateModeOption]}
					>
						{bitrateModeLabel(bitrateModeOption)}
					</option>
				{/each}
			</select>
		</div>

		<div class="encoder-field-row">
			<label for="output-quality" id="quality-bitrate-label">{encoderPanelState.qualityBitrateLabel}</label>
			<div class="encoder-field-stack">
				<select
					id="output-quality"
					class:hidden={!encoderPanelState.showQuality}
					data-testid="quality-select"
					value={encoderPanelState.qualityValue}
					onchange={handleQualityValueChange}
				>
					{#each encoderPanelState.qualityOptions as qualityOption}
						<option value={qualityOption}>{qualityLabel(qualityOption)}</option>
					{/each}
				</select>
				<select
					id="output-bitrate"
					class:hidden={encoderPanelState.showQuality}
					data-testid="bitrate-select"
					value={encoderPanelState.bitrateValue}
					onchange={handleBitrateValueChange}
				>
					{#each encoderPanelState.bitrateOptions as bitrateOption}
						<option value={bitrateOption}>{bitrateOption} kbps</option>
					{/each}
				</select>
				<p id="estimated-bitrate" class="field-hint" data-testid="estimated-bitrate">
					{encoderPanelState.estimatedBitrateText}
				</p>
			</div>
		</div>

		<div
			id="encoder-inline-option-row"
			class="encoder-inline-option-row encoder-option-row-workbench"
			class:hidden={!encoderPanelState.showInlineOptionRow}
			data-testid="encoder-inline-option-row"
		>
			<div
				id="fdk-options"
				class="encoder-option-group"
				class:hidden={!encoderPanelState.showFdkOptions}
				data-testid="fdk-options"
			>
				<label
					class="checkbox-label encoder-inline-toggle"
					data-testid="afterburner-toggle"
					title="Enable Afterburner"
				>
					<input
						type="checkbox"
						id="adv-fdk-afterburner"
						data-testid="afterburner-checkbox"
						checked={encoderPanelState.fdkAfterburner}
						onchange={handleFdkAfterburnerChange}
					/>
					<span class="option-label">Afterburner</span>
				</label>
			</div>

			<div
				id="native-options"
				class="encoder-option-group"
				class:hidden={!encoderPanelState.showNativeOptions}
				data-testid="native-options"
			>
				<label
					class="checkbox-label encoder-inline-toggle"
					data-testid="twoloop-toggle"
					title="Enable Twoloop (High Quality)"
				>
					<input
						type="checkbox"
						id="adv-native-twoloop"
						data-testid="twoloop-checkbox"
						checked={encoderPanelState.nativeTwoloop}
						onchange={handleNativeTwoloopChange}
					/>
					<span class="option-label">Twoloop</span>
				</label>
			</div>
		</div>

		<div class="encoder-field-row">
			<label for="output-samplerate">Sample Rate</label>
			<div class="encoder-field-stack">
				<select
					id="output-samplerate"
					data-testid="samplerate-select"
					value={encoderPanelState.sampleRateSelection}
					disabled={encoderPanelState.sampleRateOptions.length === 0}
					onchange={handleSampleRateSelectionChange}
				>
					{#each encoderPanelState.sampleRateOptions as sampleRateOption}
						<option value={sampleRateOption}>{sampleRateLabel(sampleRateOption)}</option>
					{/each}
				</select>
				<p id="output-samplerate-effective" class="field-hint" data-testid="auto-samplerate-hint">
					{sampleRateDetailText()}
				</p>
			</div>
		</div>

		<div class="encoder-field-row">
			<label for="output-channels">Channels</label>
			<div class="encoder-field-stack">
				<select
					id="output-channels"
					data-testid="channels-select"
					value={encoderPanelState.channelsSelection}
					disabled={encoderPanelState.channelOptions.length === 0}
					onchange={handleChannelsSelectionChange}
				>
					{#each encoderPanelState.channelOptions as channelOption}
						<option value={channelOption}>{channelLabel(channelOption)}</option>
					{/each}
				</select>
				<p id="output-channels-effective" class="field-hint" data-testid="auto-channels-hint">
					{channelsDetailText()}
				</p>
			</div>
		</div>
	</div>
</div>

<style>
	.encoder-workbench-panel {
		min-width: 0;
	}

	.encoder-workbench-header {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: 0.5rem;
		margin-bottom: 0.45rem;
	}

	.encoder-workbench-header h3 {
		margin: 0;
		color: var(--text-secondary);
		font-size: 0.875rem;
		font-weight: 600;
	}

	.encoder-workbench-grid {
		display: grid;
		grid-template-columns: 1fr;
		gap: 0.38rem;
	}

	.encoder-field-row {
		display: grid;
		grid-template-columns: minmax(4.625rem, 0.38fr) minmax(0, 1fr);
		align-items: start;
		gap: 0.55rem;
		min-width: 0;
	}

	.encoder-field-row > label,
	.encoder-field-row > .label {
		margin: 0.45rem 0 0;
		line-height: 1.2;
	}

	.encoder-field-stack {
		min-width: 0;
	}

	.encoder-workbench-panel :global(select) {
		margin-top: 0;
		min-height: 1.95rem;
	}

	.profile-display-workbench {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.5rem;
		min-height: 1.95rem;
		padding: 0.25rem 0.45rem 0.25rem 0.55rem;
		border: 1px solid var(--border-primary);
		border-radius: 0.375rem;
		background-color: var(--bg-input);
		color: var(--text-secondary);
		font-size: 0.8125rem;
	}

	.profile-display-workbench #encoder-profile-display {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.readonly-badge {
		flex: 0 0 auto;
		padding: 0.125rem 0.35rem;
		border: 1px solid var(--border-secondary);
		border-radius: 0.25rem;
		color: var(--text-muted);
		font-size: 0.6875rem;
		line-height: 1.2;
	}

	.field-hint {
		margin: 0.125rem 0 0;
		color: var(--text-muted);
		font-size: 0.7rem;
		line-height: 1.25;
	}

	.encoder-option-row-workbench {
		grid-column: 1 / -1;
		display: flex;
		align-items: center;
		gap: 0.75rem;
		min-height: 0;
	}

	.encoder-inline-toggle {
		margin-top: 0;
		align-items: center;
	}
</style>
