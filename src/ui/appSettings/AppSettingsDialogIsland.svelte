<script lang="ts">
	import { onDestroy } from 'svelte';
	import type { AppSettings, PinnedDefaults } from '../../types/appSettings';
	import { ModalController } from '../../lib/ui/modal.svelte';
	import { readFdkAfterburner, setFdkAfterburner } from '../encoderPanel';
	import {
		appSettingsDialogState,
		browseForFfmpegBinary,
		clearFfmpegPathDraft,
		closeAppSettingsDialog,
		resetAllAppSettings,
		saveCurrentSettingsAsPinnedDefaults,
		saveToolchainPreference,
		setStartupBehavior,
	} from './settingsDialog.svelte';

	let dialogEl = $state<HTMLElement | null>(null);
	const modal = new ModalController();
	let resetConfirming = $state(false);
	let resetConfirmTimeout: ReturnType<typeof setTimeout> | undefined;

	$effect(() => {
		modal.sync(appSettingsDialogState.isOpen, { container: dialogEl }, { onEscape: closeAppSettingsDialog });
	});
	onDestroy(() => modal.destroy());

	function handleAfterburnerChange(event: Event): void {
		const target = (event.currentTarget ?? event.target) as HTMLInputElement | null;
		void setFdkAfterburner(Boolean(target?.checked));
	}

	function cancelResetConfirm(): void {
		clearTimeout(resetConfirmTimeout);
		resetConfirmTimeout = undefined;
		resetConfirming = false;
	}

	function requestResetConfirm(): void {
		resetConfirming = true;
		clearTimeout(resetConfirmTimeout);
		resetConfirmTimeout = setTimeout(cancelResetConfirm, 4000);
	}

	function confirmReset(): void {
		cancelResetConfirm();
		void resetAllAppSettings();
	}

	function handleWindowClickForResetConfirm(event: MouseEvent): void {
		if (!resetConfirming) return;
		const target = event.target;
		if (target instanceof Element && target.closest('[data-testid="app-settings-reset-row"]')) return;
		cancelResetConfirm();
	}

	$effect(() => {
		if (!appSettingsDialogState.isOpen) cancelResetConfirm();
	});

	function handleBackdropClick(event: MouseEvent): void {
		if (event.target === event.currentTarget) {
			closeAppSettingsDialog();
		}
	}

	function formatConcurrency(settings: Pick<AppSettings, 'maxConcurrentJobs'>): string {
		const preference = settings.maxConcurrentJobs;
		return preference.mode === 'fixed' ? `Fixed (${preference.value})` : 'Auto';
	}

	function formatEncoderDefaults(settings: Pick<AppSettings, 'encoderDefaults'>): string {
		const encoder = settings.encoderDefaults.settings;
		const mode =
			encoder.bitrateMode.mode === 'vbr'
				? `VBR ${encoder.bitrateMode.value}`
				: encoder.bitrateMode.mode.toUpperCase();
		return `${formatEncoderType(encoder.encoderType)} • ${encoder.bitrateKbps} kbps • ${mode}`;
	}

	const pinnedDefaults = $derived<PinnedDefaults | undefined>(
		appSettingsDialogState.settings?.pinnedDefaults,
	);
	const startupBehavior = $derived(
		appSettingsDialogState.settings?.startupBehavior ?? 'rememberLastState',
	);

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

	function formatOutputDefaults(settings: Pick<AppSettings, 'outputDefaults'>): string {
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

<svelte:window onclick={handleWindowClickForResetConfirm} />

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
		bind:this={dialogEl}
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
					<label class="checkbox-label" data-testid="app-settings-afterburner-toggle">
						<input
							type="checkbox"
							id="app-settings-afterburner"
							data-testid="app-settings-afterburner-checkbox"
							checked={readFdkAfterburner()}
							onchange={handleAfterburnerChange}
						/>
						<span class="option-label">FDK Afterburner</span>
					</label>
					<p class="text-xs muted-text">
						Extra encoding effort for slightly higher quality on the FDK encoder.
						Leave on unless encode speed matters more than quality.
					</p>
				</section>

				{#if appSettingsDialogState.settings}
					<section class="app-settings-section">
						<h4 class="app-settings-section-title">Startup settings</h4>
						<p class="text-xs muted-text">
							The encoder, output, and job controls save as you change them. This
							chooses what the app restores on launch.
						</p>
						<div class="app-settings-startup-options" role="radiogroup" aria-label="On launch">
							<label class="app-settings-radio">
								<input
									type="radio"
									name="app-settings-startup-behavior"
									value="rememberLastState"
									data-testid="app-settings-startup-last"
									checked={startupBehavior === 'rememberLastState'}
									onchange={() => void setStartupBehavior('rememberLastState')}
								/>
								Remember my last settings
							</label>
							<label class="app-settings-radio">
								<input
									type="radio"
									name="app-settings-startup-behavior"
									value="pinnedDefaults"
									data-testid="app-settings-startup-pinned"
									checked={startupBehavior === 'pinnedDefaults'}
									disabled={!pinnedDefaults}
									onchange={() => void setStartupBehavior('pinnedDefaults')}
								/>
								Use my pinned defaults
								{#if !pinnedDefaults}
									<span class="text-xs muted-text">(pin defaults first)</span>
								{/if}
							</label>
						</div>
						<div class="app-settings-path-row">
							<button
								class="btn-pill btn-pill-secondary"
								data-testid="app-settings-pin-defaults"
								type="button"
								disabled={appSettingsDialogState.startupSaveState === 'saving'}
								onclick={() => void saveCurrentSettingsAsPinnedDefaults()}
							>
								Use current settings as defaults
							</button>
						</div>
						{#if appSettingsDialogState.startupSaveState === 'error'}
							<p
								class="app-settings-status app-settings-status-error"
								data-testid="app-settings-startup-error"
							>
								{appSettingsDialogState.startupSaveError}
							</p>
						{/if}
						<dl class="app-settings-summary" data-testid="app-settings-summary">
							{#if pinnedDefaults}
								<dt>Pinned max jobs</dt>
								<dd>{formatConcurrency(pinnedDefaults)}</dd>
								<dt>Pinned encoder</dt>
								<dd>{formatEncoderDefaults(pinnedDefaults)}</dd>
								<dt>Pinned output folder</dt>
								<dd>{formatOutputDefaults(pinnedDefaults)}</dd>
							{:else}
								<dt>Pinned defaults</dt>
								<dd data-testid="app-settings-no-pin">Not pinned yet</dd>
							{/if}
						</dl>
					</section>

					<section class="app-settings-section">
						<h4 class="app-settings-section-title">Reset</h4>
						<div class="app-settings-path-row" data-testid="app-settings-reset-row">
							{#if resetConfirming}
								<span class="text-xs muted-text" data-testid="app-settings-reset-confirm-prompt">
									Reset all settings?
								</span>
								<button
									data-testid="app-settings-reset-confirm"
									type="button"
									disabled={appSettingsDialogState.saveState === 'saving'}
									onclick={confirmReset}
								>
									Reset
								</button>
								<button
									data-testid="app-settings-reset-cancel"
									type="button"
									onclick={cancelResetConfirm}
								>
									Cancel
								</button>
							{:else}
								<button
									data-testid="app-settings-reset"
									type="button"
									disabled={appSettingsDialogState.saveState === 'saving'}
									onclick={requestResetConfirm}
								>
									Reset all settings to defaults
								</button>
							{/if}
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

	.app-settings-startup-options {
		display: flex;
		flex-direction: column;
		gap: 0.3rem;
		margin-top: 0.5rem;
		font-size: 0.78rem;
		color: var(--text-primary);
	}

	.app-settings-radio {
		display: inline-flex;
		align-items: center;
		gap: 0.4rem;
		cursor: pointer;
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
