<script lang="ts">
	import type { AppSettings } from '../../types/appSettings';
	import {
		appSettingsDialogState,
		browseForFfmpegBinary,
		clearFfmpegPathDraft,
		closeAppSettingsDialog,
		resetAllAppSettings,
		saveToolchainPreference,
	} from './settingsDialog.svelte';

	function handleBackdropClick(event: MouseEvent): void {
		if (event.target === event.currentTarget) {
			closeAppSettingsDialog();
		}
	}

	function formatConcurrency(settings: AppSettings): string {
		const preference = settings.maxConcurrentJobs;
		return preference.mode === 'fixed' ? `Fixed (${preference.value})` : 'Auto';
	}

	function formatEncoderDefaults(settings: AppSettings): string {
		const encoder = settings.encoderDefaults.settings;
		const mode =
			encoder.bitrateMode.mode === 'vbr'
				? `VBR ${encoder.bitrateMode.value}`
				: encoder.bitrateMode.mode.toUpperCase();
		return `${formatEncoderType(encoder.encoderType)} • ${encoder.bitrateKbps} kbps • ${mode}`;
	}

	function formatEncoderType(encoderType: string): string {
		switch (encoderType) {
			case 'auto':
				return 'Auto';
			case 'fdk_he_aac':
				return 'FDK HE-AAC';
			case 'aac_at':
				return 'Apple AAC';
			case 'native_aac':
				return 'Native AAC';
			default:
				return encoderType;
		}
	}

	function formatOutputDefaults(settings: AppSettings): string {
		return settings.outputDefaults.outputDirectory ?? 'Not set';
	}

	function formatFdkSource(source: string): string {
		switch (source) {
			case 'user_configured':
				return 'user-configured path';
			case 'detected':
				return 'auto-detected';
			case 'bundled':
				return 'bundled';
			default:
				return 'unavailable';
		}
	}
</script>

<div
	id="app-settings-modal"
	class="app-modal-backdrop"
	class:open={appSettingsDialogState.isOpen}
	data-testid="app-settings-modal"
	aria-hidden={!appSettingsDialogState.isOpen}
	onclick={handleBackdropClick}
>
	<div
		class="app-modal-dialog"
		role="dialog"
		aria-modal="true"
		aria-labelledby="app-settings-title"
	>
		<div class="app-modal-header">
			<h3 id="app-settings-title">App Settings</h3>
			<button
				id="app-settings-close"
				class="btn-pill btn-pill-secondary"
				data-testid="app-settings-close"
				type="button"
				onclick={closeAppSettingsDialog}
			>
				Close
			</button>
		</div>

		<div class="app-modal-body">
			{#if appSettingsDialogState.loading}
				<p class="text-xs muted-text">Loading settings…</p>
			{:else}
				<section class="app-settings-section">
					<h4 class="app-settings-section-title">External FFmpeg (FDK AAC)</h4>
					<p class="text-xs muted-text">
						Point AudioBook Boss at an FFmpeg binary built with libfdk_aac to unlock
						the FDK HE-AAC encoder. The path is validated before it is used.
					</p>
					<div class="app-settings-path-row">
						<input
							id="app-settings-ffmpeg-path"
							class="app-settings-path-input"
							data-testid="app-settings-ffmpeg-path"
							type="text"
							placeholder="/opt/homebrew/bin/ffmpeg"
							bind:value={appSettingsDialogState.ffmpegPathDraft}
						/>
						<button
							class="btn-pill btn-pill-secondary"
							data-testid="app-settings-ffmpeg-browse"
							type="button"
							onclick={() => void browseForFfmpegBinary()}
						>
							Browse…
						</button>
						<button
							class="btn-pill btn-pill-secondary"
							data-testid="app-settings-ffmpeg-clear"
							type="button"
							onclick={clearFfmpegPathDraft}
						>
							Clear
						</button>
						<button
							class="btn-pill btn-pill-primary"
							data-testid="app-settings-ffmpeg-save"
							type="button"
							disabled={appSettingsDialogState.saveState === 'saving'}
							onclick={() => void saveToolchainPreference()}
						>
							{appSettingsDialogState.saveState === 'saving' ? 'Saving…' : 'Save'}
						</button>
					</div>
					{#if appSettingsDialogState.saveState === 'error'}
						<p class="app-settings-status app-settings-status-error" data-testid="app-settings-error">
							{appSettingsDialogState.saveError}
						</p>
					{/if}
					{#if appSettingsDialogState.encoderAvailability}
						<p class="app-settings-status" data-testid="app-settings-toolchain-status">
							{appSettingsDialogState.encoderAvailability.statusMessage}
							{#if appSettingsDialogState.encoderAvailability.fdkAvailable}
								(FDK source: {formatFdkSource(appSettingsDialogState.encoderAvailability.fdkSource)})
							{/if}
						</p>
					{/if}
				</section>

				{#if appSettingsDialogState.settings}
					<section class="app-settings-section">
						<h4 class="app-settings-section-title">Durable defaults</h4>
						<p class="text-xs muted-text">
							Saved automatically as you change the encoder, output, and job controls
							in the main window; applied on every launch.
						</p>
						<dl class="app-settings-summary" data-testid="app-settings-summary">
							<dt>Max concurrent jobs</dt>
							<dd>{formatConcurrency(appSettingsDialogState.settings)}</dd>
							<dt>Encoder defaults</dt>
							<dd>{formatEncoderDefaults(appSettingsDialogState.settings)}</dd>
							<dt>Output folder</dt>
							<dd>{formatOutputDefaults(appSettingsDialogState.settings)}</dd>
						</dl>
					</section>

					<section class="app-settings-section">
						<h4 class="app-settings-section-title">Reset</h4>
						<div class="app-settings-path-row">
							<button
								class="btn-pill btn-pill-secondary"
								data-testid="app-settings-reset"
								type="button"
								disabled={appSettingsDialogState.saveState === 'saving'}
								onclick={() => void resetAllAppSettings()}
							>
								Reset all settings to defaults
							</button>
						</div>
					</section>
				{/if}
			{/if}
		</div>
	</div>
</div>

<style>
	.app-settings-section {
		margin-top: 1rem;
	}

	.app-settings-section:first-child {
		margin-top: 0;
	}

	.app-settings-section-title {
		margin: 0 0 0.35rem;
		font-size: 0.85rem;
		font-weight: 600;
		color: var(--text-primary);
	}

	.app-settings-path-row {
		display: flex;
		gap: 0.5rem;
		align-items: center;
		margin-top: 0.5rem;
	}

	.app-settings-path-input {
		flex: 1;
		min-width: 0;
		font-family: var(--font-mono);
		font-size: 0.78rem;
	}

	.app-settings-status {
		margin-top: 0.5rem;
		font-size: 0.76rem;
		color: var(--text-secondary);
	}

	.app-settings-status-error {
		color: var(--danger, #e5484d);
	}

	.app-settings-summary {
		display: grid;
		grid-template-columns: auto 1fr;
		gap: 0.3rem 1rem;
		margin: 0.5rem 0 0;
		font-size: 0.78rem;
	}

	.app-settings-summary dt {
		color: var(--text-secondary);
	}

	.app-settings-summary dd {
		margin: 0;
		color: var(--text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
