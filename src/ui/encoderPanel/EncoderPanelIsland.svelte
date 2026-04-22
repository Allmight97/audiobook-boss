<script lang="ts">
	import { onMount } from 'svelte';
	import { encoderPanelState } from './state.svelte';
	import { outputPanelState } from '../outputPanel/state.svelte';
	import {
		clearToolchainOverride,
		handleBitrateModeChange,
		handleBitrateValueChange,
		handleToolchainBrowse,
		handleChannelsSelectionChange,
		handleFlavorChange,
		handleFdkAfterburnerChange,
		handleNativeTwoloopChange,
		handleQualityValueChange,
		handleSampleRateSelectionChange,
		handleToolchainPathCommit,
		handleToolchainPathInput,
		initializeEncoderPanelLogic,
		refreshExternalToolchain,
	} from './logic';

	onMount(() => {
		initializeEncoderPanelLogic();
	});
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
				on:change={handleFlavorChange}
			>
				<option value="auto">{encoderPanelState.autoOptionLabel}</option>
				<option value="fdk_he_aac" disabled={encoderPanelState.disabledEncoderOptions.fdk_he_aac}>
					FDK AAC
				</option>
				<option value="aac_at" disabled={encoderPanelState.disabledEncoderOptions.aac_at}>
					Apple AAC
				</option>
				<option
					value="native_aac"
					disabled={encoderPanelState.disabledEncoderOptions.native_aac}
				>
					Native AAC (FFmpeg)
				</option>
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
				on:change={handleBitrateModeChange}
			>
				<option value="vbr" disabled={!encoderPanelState.bitrateModeAvailability.vbr}>VBR</option>
				<option value="cvbr" disabled={!encoderPanelState.bitrateModeAvailability.cvbr}>CVBR</option>
				<option value="cbr" disabled={!encoderPanelState.bitrateModeAvailability.cbr}>CBR</option>
			</select>
		</div>
		<div>
			<label for="output-quality" id="quality-bitrate-label">{encoderPanelState.qualityBitrateLabel}</label>
			<select
				id="output-quality"
				class:hidden={!encoderPanelState.showQuality}
				data-testid="quality-select"
				value={encoderPanelState.qualityValue}
				on:change={handleQualityValueChange}
			>
				<option value={1}>1 (Smallest)</option>
				<option value={2}>2</option>
				<option value={3}>3 (Recommended)</option>
				<option value={4}>4</option>
				<option value={5}>5 (Largest)</option>
			</select>
			<select
				id="output-bitrate"
				class:hidden={encoderPanelState.showQuality}
				data-testid="bitrate-select"
				value={encoderPanelState.bitrateValue}
				on:change={handleBitrateValueChange}
			>
				<option value={48}>48 kbps</option>
				<option value={56}>56 kbps</option>
				<option value={64}>64 kbps</option>
				<option value={72}>72 kbps</option>
				<option value={80}>80 kbps</option>
				<option value={88}>88 kbps</option>
				<option value={96}>96 kbps</option>
				<option value={104}>104 kbps</option>
				<option value={112}>112 kbps</option>
				<option value={120}>120 kbps</option>
				<option value={128}>128 kbps</option>
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
					on:change={handleFdkAfterburnerChange}
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
					on:change={handleNativeTwoloopChange}
				/>
				<span class="option-label">Twoloop</span>
			</label>
		</div>
	</div>

	<div class="toolchain-status-card" data-testid="external-toolchain-panel">
		<div class="toolchain-status-main">
			<div class="toolchain-status-copy">
				<span class="label">FDK AAC</span>
				<p
					id="external-toolchain-status"
					class="text-xs muted-text mt-0.5"
					data-testid="external-toolchain-status"
				>
					<strong>{encoderPanelState.toolchainStatusTitle}</strong>
					<span> {encoderPanelState.toolchainStatusMessage}</span>
				</p>
				{#if encoderPanelState.toolchainActivePath}
					<p
						id="external-toolchain-path-display"
						class="text-xs muted-text mt-0.5 toolchain-path"
						data-testid="external-toolchain-path-display"
					>
						{encoderPanelState.toolchainActivePath}
					</p>
				{/if}
				{#if encoderPanelState.toolchainOverrideError}
					<p
						id="external-toolchain-error"
						class="text-xs mt-0.5 toolchain-error-text"
						data-testid="external-toolchain-error"
					>
						{encoderPanelState.toolchainOverrideError}
					</p>
				{/if}
			</div>
			<div class="toolchain-action-row">
				<button
					id="toolchain-refresh"
					type="button"
					class="secondary-button compact-button"
					on:click={refreshExternalToolchain}
					data-testid="toolchain-refresh"
				>
					Refresh
				</button>
				{#if encoderPanelState.externalToolchainOverridePath.trim()}
					<button
						id="toolchain-clear-override"
						type="button"
						class="secondary-button compact-button"
						on:click={clearToolchainOverride}
						data-testid="toolchain-clear-override"
					>
						Clear Path
					</button>
				{/if}
			</div>
		</div>

		{#if encoderPanelState.showToolchainOverrideInput}
			<div class="toolchain-override-row">
				<div class="toolchain-override-input">
					<label for="external-toolchain-path">ffmpeg Path</label>
					<input
						id="external-toolchain-path"
						type="text"
						placeholder="/opt/homebrew/bin/ffmpeg or /path/to/toolchain"
						value={encoderPanelState.externalToolchainOverridePath}
						data-testid="external-toolchain-path"
						on:input={handleToolchainPathInput}
						on:change={handleToolchainPathCommit}
						on:blur={handleToolchainPathCommit}
					/>
					<p class="text-xs muted-text mt-0.5">
						Paste an `ffmpeg` executable path or a toolchain directory containing `ffmpeg`.
					</p>
				</div>
				<div class="toolchain-action-row toolchain-override-actions">
					<button
						id="toolchain-browse"
						type="button"
						class="secondary-button compact-button"
						on:click={handleToolchainBrowse}
						data-testid="toolchain-browse"
					>
						Choose…
					</button>
				</div>
			</div>
		{/if}
	</div>

	<div class="grid grid-cols-4 gap-x-3 gap-y-2 mb-2">
		<div>
			<label for="output-samplerate">Sample Rate</label>
			<select
				id="output-samplerate"
				data-testid="samplerate-select"
				value={encoderPanelState.sampleRateSelection}
				on:change={handleSampleRateSelectionChange}
			>
				<option value="auto">Auto</option>
				<option value="22050">22050 Hz</option>
				<option value="32000">32000 Hz</option>
				<option value="44100">44100 Hz</option>
				<option value="48000">48000 Hz</option>
			</select>
			<p
				id="output-samplerate-effective"
				class="text-xs muted-text mt-0.5"
				data-testid="auto-samplerate-hint"
			>
				{encoderPanelState.sampleRateAutoHint}
			</p>
		</div>
		<div>
			<label for="output-channels">Channels</label>
			<select
				id="output-channels"
				data-testid="channels-select"
				value={encoderPanelState.channelsSelection}
				on:change={handleChannelsSelectionChange}
			>
				<option value="auto">Auto</option>
				<option value="mono">Mono</option>
				<option value="stereo">Stereo</option>
			</select>
			<p
				id="output-channels-effective"
				class="text-xs muted-text mt-0.5"
				data-testid="auto-channels-hint"
			>
				{encoderPanelState.channelsAutoHint}
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

	.secondary-button.compact-button {
		margin-top: 0;
		padding: 0.35rem 0.65rem;
		border: 1px solid var(--border-secondary);
		border-radius: 0.375rem;
		background: var(--bg-input);
		color: var(--text-secondary);
		font-size: 0.75rem;
	}

	.toolchain-status-card {
		margin: 0.25rem 0 0.75rem;
		padding: 0.625rem 0.75rem;
		border: 1px solid var(--border-primary);
		border-radius: 0.5rem;
		background: color-mix(in srgb, var(--bg-input) 78%, var(--bg-drag-area) 22%);
	}

	.toolchain-status-main {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 0.75rem;
	}

	.toolchain-status-copy {
		flex: 1;
		min-width: 0;
	}

	.toolchain-path {
		font-family: var(--font-mono);
		word-break: break-all;
	}

	.toolchain-error-text {
		color: #dc2626;
	}

	.toolchain-action-row {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.5rem;
	}

	.toolchain-override-row {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 0.75rem;
		margin-top: 0.625rem;
	}

	.toolchain-override-input {
		flex: 1;
		min-width: 0;
	}

	.toolchain-override-actions {
		padding-top: 1.5rem;
	}

	.encoder-inline-toggle {
		margin-top: 0;
		align-items: center;
	}
</style>
