<script lang="ts">
	import { onMount } from 'svelte';
	import { encoderPanelState } from './state.svelte';
	import { outputPanelState } from '../outputPanel/state.svelte';
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

	onMount(() => {
		initializeEncoderPanelLogic();
	});

	function encoderLabel(value: string): string {
		switch (value) {
			case 'auto':
				return encoderPanelState.autoOptionLabel;
			case 'fdk_he_aac':
				return 'FDK AAC';
			case 'aac_at':
				return 'Apple AAC';
			case 'native_aac':
				return 'Native AAC (FFmpeg)';
			default:
				return value;
		}
	}

	function bitrateModeLabel(value: string): string {
		return value.toUpperCase();
	}

	function qualityLabel(value: number): string {
		if (value === encoderPanelState.capabilities?.vbrLevelMin) return `${value} (Smallest)`;
		if (value === encoderPanelState.capabilities?.vbrLevelDefault) return `${value} (Recommended)`;
		if (value === encoderPanelState.capabilities?.vbrLevelMax) return `${value} (Largest)`;
		return String(value);
	}

	function sampleRateLabel(value: string): string {
		return value === 'auto' ? 'Auto' : `${value} Hz`;
	}

	function channelLabel(value: string): string {
		switch (value) {
			case 'auto':
				return 'Auto';
			case 'mono':
				return 'Mono';
			case 'stereo':
				return 'Stereo';
			default:
				return value;
		}
	}

	function sampleRateDetailText(): string {
		if (encoderPanelState.sampleRateSelection === 'auto') {
			return encoderPanelState.sampleRateAutoHint;
		}
		return `Using ${sampleRateLabel(encoderPanelState.sampleRateSelection)}.`;
	}

	function channelsDetailText(): string {
		if (encoderPanelState.channelsSelection === 'auto') {
			return encoderPanelState.channelsAutoHint;
		}
		return `Using ${channelLabel(encoderPanelState.channelsSelection ?? 'auto')}.`;
	}

	function encoderOptionDisabled(value: string): boolean {
		if (value === 'auto') return false;
		return Boolean(
			encoderPanelState.disabledEncoderOptions[
				value as keyof typeof encoderPanelState.disabledEncoderOptions
			],
		);
	}
</script>

<div id="encoder-settings-panel" class="section-divider" data-testid="encoder-settings-panel">
	<div class="section-header">
		<h3>Audio Encoder Settings</h3>
		<span class="inline-info">
			(<span id="estimated-size" data-testid="estimated-size">{outputPanelState.estimatedSizeText}</span>)
		</span>
	</div>

	<div class="grid grid-cols-4 gap-x-3 gap-y-2 mb-2">
		<div>
			<label for="adv-encoder">Encoder</label>
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
				class="text-xs muted-text mt-0.5"
				data-testid="encoder-availability-hint"
			>
				{encoderPanelState.availabilityHint}
			</p>
		</div>
		<div>
			<span class="label">Profile</span>
			<div id="encoder-profile-display" class="profile-display" data-testid="profile-display">
				{encoderPanelState.profileDisplay}
			</div>
			<p class="text-xs muted-text mt-0.5 italic">Read-only</p>
		</div>
		<div>
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
		<div>
			<label for="output-quality" id="quality-bitrate-label">{encoderPanelState.qualityBitrateLabel}</label>
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
			<p id="estimated-bitrate" class="text-xs muted-text mt-0.5" data-testid="estimated-bitrate">
				{encoderPanelState.estimatedBitrateText}
			</p>
		</div>
	</div>

	<div
		id="encoder-inline-option-row"
		class="encoder-inline-option-row"
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

	<div class="grid grid-cols-4 gap-x-3 gap-y-2 mb-2">
		<div>
			<label for="output-samplerate">Sample Rate</label>
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
			<p
				id="output-samplerate-effective"
				class="text-xs muted-text mt-0.5"
				data-testid="auto-samplerate-hint"
			>
				{sampleRateDetailText()}
			</p>
		</div>
		<div>
			<label for="output-channels">Channels</label>
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
			<p
				id="output-channels-effective"
				class="text-xs muted-text mt-0.5"
				data-testid="auto-channels-hint"
			>
				{channelsDetailText()}
			</p>
		</div>
	</div>
</div>

<style>
	.profile-display {
		padding: 0.5rem 0.75rem;
		border: 1px solid var(--border-primary);
		border-radius: 0.375rem;
		background-color: var(--bg-input);
		color: var(--text-secondary);
		font-size: 0.875rem;
	}

	.encoder-inline-option-row {
		display: flex;
		align-items: center;
		gap: 1rem;
		margin: 0.125rem 0 0.5rem;
	}

	.encoder-option-group {
		padding: 0;
	}

	.encoder-inline-toggle {
		margin-top: 0;
		align-items: center;
	}
</style>
