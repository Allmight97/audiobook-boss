import { createSignal, onCleanup, onMount, Show, type JSX } from 'solid-js';
import type { AppSettings, PinnedDefaults } from '../../types/appSettings';
import {
	browseForFfmpegBinary,
	clearFfmpegPathDraft,
	closeProductionSettingsDialog,
	productionSettingsDialogState,
	resetAllAppSettings,
	saveCurrentSettingsAsPinnedDefaults,
	saveToolchainPreference,
	setFfmpegPathDraft,
	setProductionFdkAfterburner,
	setStartupBehavior,
	subscribeProductionSettingsDialog,
} from '../../app/appSettings';
import { Dialog } from '../../lib/ui/Dialog';
import { readFdkAfterburner } from '../encoderPanel/state';
import './appSettingsDialog.css';

const RESET_CONFIRM_MS = 4000;

function formatConcurrency(settings: Pick<AppSettings, 'maxConcurrentJobs'>): string {
	const preference = settings.maxConcurrentJobs;
	return preference.mode === 'fixed' ? `Fixed (${preference.value})` : 'Auto';
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

function formatEncoderDefaults(settings: Pick<AppSettings, 'encoderDefaults'>): string {
	const encoder = settings.encoderDefaults.settings;
	const mode =
		encoder.bitrateMode.mode === 'vbr'
			? `VBR ${encoder.bitrateMode.value}`
			: encoder.bitrateMode.mode.toUpperCase();
	return `${formatEncoderType(encoder.encoderType)} • ${encoder.bitrateKbps} kbps • ${mode}`;
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

export function AppSettingsDialogView(): JSX.Element {
	const [revision, setRevision] = createSignal(0);
	const [resetConfirming, setResetConfirming] = createSignal(false);
	let resetConfirmTimeout: ReturnType<typeof setTimeout> | undefined;

	const state = () => {
		revision();
		return productionSettingsDialogState;
	};

	function cancelResetConfirm(): void {
		clearTimeout(resetConfirmTimeout);
		resetConfirmTimeout = undefined;
		setResetConfirming(false);
	}

	function requestResetConfirm(): void {
		setResetConfirming(true);
		clearTimeout(resetConfirmTimeout);
		resetConfirmTimeout = setTimeout(cancelResetConfirm, RESET_CONFIRM_MS);
	}

	function confirmReset(): void {
		cancelResetConfirm();
		void resetAllAppSettings();
	}

	function handleWindowClickForResetConfirm(event: MouseEvent): void {
		if (!resetConfirming()) return;
		const target = event.target;
		if (target instanceof Element && target.closest('[data-testid="app-settings-reset-row"]')) {
			return;
		}
		cancelResetConfirm();
	}

	onMount(() => {
		const unsubscribe = subscribeProductionSettingsDialog(() => {
			setRevision((value) => value + 1);
			if (!productionSettingsDialogState.isOpen) cancelResetConfirm();
		});
		window.addEventListener('click', handleWindowClickForResetConfirm, true);
		onCleanup(() => {
			unsubscribe();
			window.removeEventListener('click', handleWindowClickForResetConfirm, true);
			cancelResetConfirm();
		});
	});

	const pinnedDefaults = (): PinnedDefaults | undefined => state().settings?.pinnedDefaults;
	const startupBehavior = () => state().settings?.startupBehavior ?? 'rememberLastState';

	return (
		<Dialog
			id="app-settings-modal"
			open={state().isOpen}
			onClose={closeProductionSettingsDialog}
			labelledBy="app-settings-title"
			testId="app-settings-modal"
		>
			<div class="app-modal-header">
				<h3 id="app-settings-title">App Settings</h3>
				<button
					id="app-settings-close"
					class="btn-pill btn-pill-secondary"
					data-testid="app-settings-close"
					type="button"
					onClick={closeProductionSettingsDialog}
				>
					Close
				</button>
			</div>
			<div class="app-modal-body">
				<Show
					when={!state().loading}
					fallback={<p class="text-xs muted-text">Loading settings…</p>}
				>
					<section class="app-settings-section">
						<h4 class="app-settings-section-title">External FFmpeg (FDK AAC)</h4>
						<p class="text-xs muted-text">
							Point AudioBook Boss at an FFmpeg binary built with libfdk_aac to unlock the FDK
							HE-AAC encoder. The path is validated before it is used.
						</p>
						<div class="app-settings-path-row">
							<input
								id="app-settings-ffmpeg-path"
								class="app-settings-path-input"
								data-testid="app-settings-ffmpeg-path"
								type="text"
								placeholder="/opt/homebrew/bin/ffmpeg"
								value={state().ffmpegPathDraft}
								onInput={(event) => setFfmpegPathDraft(event.currentTarget.value)}
							/>
							<button
								class="btn-pill btn-pill-secondary"
								data-testid="app-settings-ffmpeg-browse"
								type="button"
								onClick={() => void browseForFfmpegBinary()}
							>
								Browse…
							</button>
							<button
								class="btn-pill btn-pill-secondary"
								data-testid="app-settings-ffmpeg-clear"
								type="button"
								onClick={clearFfmpegPathDraft}
							>
								Clear
							</button>
							<button
								class="btn-pill btn-pill-primary"
								data-testid="app-settings-ffmpeg-save"
								type="button"
								disabled={state().saveState === 'saving'}
								onClick={() => void saveToolchainPreference()}
							>
								{state().saveState === 'saving' ? 'Saving…' : 'Save'}
							</button>
						</div>
						<Show when={state().saveState === 'error'}>
							<p
								class="app-settings-status app-settings-status-error"
								data-testid="app-settings-error"
							>
								{state().saveError}
							</p>
						</Show>
						<Show when={state().encoderAvailability}>
							{(availability) => (
								<p class="app-settings-status" data-testid="app-settings-toolchain-status">
									{availability().statusMessage}
									<Show when={availability().fdkAvailable}>
										{` (FDK source: ${formatFdkSource(availability().fdkSource)})`}
									</Show>
								</p>
							)}
						</Show>
						<label class="checkbox-label" data-testid="app-settings-afterburner-toggle">
							<input
								type="checkbox"
								id="app-settings-afterburner"
								data-testid="app-settings-afterburner-checkbox"
								checked={readFdkAfterburner()}
								onChange={(event) =>
									setProductionFdkAfterburner(Boolean(event.currentTarget.checked))
								}
							/>
							<span class="option-label">FDK Afterburner</span>
						</label>
						<p class="text-xs muted-text">
							Extra encoding effort for slightly higher quality on the FDK encoder. Leave on unless
							encode speed matters more than quality.
						</p>
					</section>
					<Show when={state().settings}>
						{(_) => (
							<>
								<section class="app-settings-section">
									<h4 class="app-settings-section-title">Startup settings</h4>
									<p class="text-xs muted-text">
										The encoder, output, and job controls save as you change them. This chooses what
										the app restores on launch.
									</p>
									<div
										class="app-settings-startup-options"
										role="radiogroup"
										aria-label="On launch"
									>
										<label class="app-settings-radio">
											<input
												type="radio"
												name="app-settings-startup-behavior"
												value="rememberLastState"
												data-testid="app-settings-startup-last"
												checked={startupBehavior() === 'rememberLastState'}
												onChange={() => void setStartupBehavior('rememberLastState')}
											/>
											Remember my last settings
										</label>
										<label class="app-settings-radio">
											<input
												type="radio"
												name="app-settings-startup-behavior"
												value="pinnedDefaults"
												data-testid="app-settings-startup-pinned"
												checked={startupBehavior() === 'pinnedDefaults'}
												disabled={!pinnedDefaults()}
												onChange={() => void setStartupBehavior('pinnedDefaults')}
											/>
											Use my pinned defaults
											<Show when={!pinnedDefaults()}>
												<span class="text-xs muted-text">(pin defaults first)</span>
											</Show>
										</label>
									</div>
									<div class="app-settings-path-row">
										<button
											class="btn-pill btn-pill-secondary"
											data-testid="app-settings-pin-defaults"
											type="button"
											disabled={state().startupSaveState === 'saving'}
											onClick={() => void saveCurrentSettingsAsPinnedDefaults()}
										>
											Use current settings as defaults
										</button>
									</div>
									<Show when={state().startupSaveState === 'error'}>
										<p
											class="app-settings-status app-settings-status-error"
											data-testid="app-settings-startup-error"
										>
											{state().startupSaveError}
										</p>
									</Show>
									<dl class="app-settings-summary" data-testid="app-settings-summary">
										<Show
											when={pinnedDefaults()}
											fallback={
												<>
													<dt>Pinned defaults</dt>
													<dd data-testid="app-settings-no-pin">Not pinned yet</dd>
												</>
											}
										>
											{(pinned) => (
												<>
													<dt>Pinned max jobs</dt>
													<dd>{formatConcurrency(pinned())}</dd>
													<dt>Pinned encoder</dt>
													<dd>{formatEncoderDefaults(pinned())}</dd>
													<dt>Pinned output folder</dt>
													<dd>{formatOutputDefaults(pinned())}</dd>
												</>
											)}
										</Show>
									</dl>
								</section>
								<section class="app-settings-section">
									<h4 class="app-settings-section-title">Reset</h4>
									<div class="app-settings-path-row" data-testid="app-settings-reset-row">
										<Show
											when={resetConfirming()}
											fallback={
												<button
													data-testid="app-settings-reset"
													type="button"
													disabled={state().saveState === 'saving'}
													onClick={requestResetConfirm}
												>
													Reset all settings to defaults
												</button>
											}
										>
											<span
												class="text-xs muted-text"
												data-testid="app-settings-reset-confirm-prompt"
											>
												Reset all settings?
											</span>
											<button
												data-testid="app-settings-reset-confirm"
												type="button"
												disabled={state().saveState === 'saving'}
												onClick={confirmReset}
											>
												Reset
											</button>
											<button
												data-testid="app-settings-reset-cancel"
												type="button"
												onClick={cancelResetConfirm}
											>
												Cancel
											</button>
										</Show>
									</div>
								</section>
							</>
						)}
					</Show>
				</Show>
			</div>
		</Dialog>
	);
}
