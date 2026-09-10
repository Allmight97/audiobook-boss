import { For, Show } from 'solid-js';
import type { JSX } from '@solidjs/web';

import { useAppRuntime } from '../../app/runtime';
import type { EncodingField } from '../../app/encoding';
import './encoderView.css';

export function EncoderView(): JSX.Element {
	const runtime = useAppRuntime();
	const view = runtime.encoding.view;
	const estimatedSizeText = runtime.output.estimatedSizeText;

	function bind(field: EncodingField) {
		return (event: Event) => {
			const select = event.currentTarget as HTMLSelectElement | HTMLInputElement;
			runtime.encoding.select(field, select.value);
			if (field === 'encoder') select.value = view().flavor;
			if (field === 'nativeQuality') select.value = String(view().nativeQuality);
			if (field === 'bitrate') select.value = String(view().bitrate);
		};
	}

	return (
		<div
			id="encoder-settings-panel"
			class="encoder-workbench-panel"
			data-testid="encoder-settings-panel"
		>
			<div class="encoder-workbench-header">
				<h3>Encoder</h3>
				<span class="inline-info">
					(
					<span id="estimated-size" data-testid="estimated-size">
						{estimatedSizeText()}
					</span>
					)
				</span>
			</div>
			<div class="encoder-workbench-grid">
				<div class="encoder-field-row">
					<label for="adv-encoder">Encoder</label>
					<div class="encoder-field-stack">
						<select
							id="adv-encoder"
							data-testid="encoder-select"
							value={view().flavor}
							disabled={view().flavorDisabled}
							onChange={bind('encoder')}
						>
							<For each={view().flavorOptions}>
								{(option) => (
									<option value={option.value} disabled={option.disabled}>
										{option.label}
									</option>
								)}
							</For>
						</select>
						<p
							id="encoder-availability-hint"
							class="field-hint"
							data-testid="encoder-availability-hint"
						>
							{view().availabilityHint}
							<Show when={view().fdkSetupNeeded}>
								{' '}
								<button
									type="button"
									class="encoder-setup-link"
									onClick={() => void runtime.settings.openDialog()}
								>
									Set up FDK…
								</button>
							</Show>
						</p>
					</div>
				</div>
				<div class="encoder-field-row">
					<span class="label">Profile</span>
					<div class="profile-display profile-display-workbench" data-testid="profile-display">
						<span id="encoder-profile-display">{view().profileDisplay}</span>
						<span class="readonly-badge">read-only</span>
					</div>
				</div>
				<div class="encoder-field-row">
					<label for="adv-bitrate-mode">Bitrate Mode</label>
					<select
						id="adv-bitrate-mode"
						data-testid="bitrate-mode-select"
						value={view().bitrateMode}
						disabled={view().bitrateModeDisabled}
						onChange={bind('bitrateMode')}
					>
						<For each={view().bitrateModeOptions}>
							{(option) => (
								<option value={option.value} disabled={option.disabled}>
									{option.label}
								</option>
							)}
						</For>
					</select>
				</div>
				<div class="encoder-field-row">
					<label
						for={
							view().bitrateMode === 'native_vbr'
								? 'native-quality'
								: view().showQuality
									? 'output-quality'
									: 'output-bitrate'
						}
						id="quality-bitrate-label"
					>
						{view().qualityBitrateLabel}
					</label>
					<div class="encoder-field-stack">
						<select
							id="output-quality"
							hidden={view().bitrateMode !== 'vbr'}
							data-testid="quality-select"
							value={view().quality}
							onChange={bind('quality')}
						>
							<For each={view().qualityOptions}>
								{(option) => <option value={option.value}>{option.label}</option>}
							</For>
						</select>
						<input
							id="output-bitrate"
							data-testid="bitrate-input"
							type="number"
							hidden={view().showQuality}
							min={view().bitrateKbpsMin}
							max={view().bitrateKbpsMax}
							step="1"
							value={view().bitrate}
							onChange={bind('bitrate')}
							aria-describedby="estimated-bitrate"
						/>
						<Show when={view().native}>
							<Show when={view().bitrateMode === 'native_vbr'}>
								<input
									id="native-quality"
									data-testid="native-quality-input"
									type="number"
									min={view().nativeQualityMin}
									max={view().nativeQualityMax}
									step="any"
									value={view().nativeQuality}
									onChange={bind('nativeQuality')}
									aria-describedby="estimated-bitrate native-rate-hint"
								/>
							</Show>
							<p id="native-rate-hint" class="field-hint">
								{view().bitrateMode === 'native_vbr'
									? `q ${view().nativeQualityMin.toFixed(4)}–${view().nativeQualityMax.toFixed(4)}; engine safety bounds, not a calibrated quality scale.`
									: 'kbps total. Maximum depends on sample rate and channel count; incompatible combinations are rejected.'}
							</p>
						</Show>
						<p id="estimated-bitrate" class="field-hint" data-testid="estimated-bitrate">
							{view().estimatedBitrateText}
						</p>
					</div>
				</div>
				<Show when={view().native}>
					<div class="encoder-field-row">
						<label for="native-speed">NMR speed</label>
						<div class="encoder-field-stack">
							<select
								id="native-speed"
								data-testid="native-speed-select"
								value={view().nativeSpeed}
								onChange={bind('nativeSpeed')}
								aria-describedby="native-speed-hint"
							>
								<For each={view().nativeSpeedOptions}>
									{(option) => <option value={option.value}>{option.label}</option>}
								</For>
							</select>
							<p id="native-speed-hint" class="field-hint">
								Higher values reduce search effort. Psychoacoustic tools use upstream defaults.
							</p>
						</div>
					</div>
				</Show>
				<div class="encoder-field-row">
					<label for="output-samplerate">Sample Rate</label>
					<div class="encoder-field-stack">
						<select
							id="output-samplerate"
							data-testid="samplerate-select"
							value={view().sampleRate}
							disabled={view().sampleRateDisabled}
							onChange={bind('sampleRate')}
						>
							<For each={view().sampleRateOptions}>
								{(option) => <option value={option.value}>{option.label}</option>}
							</For>
						</select>
						<p
							id="output-samplerate-effective"
							class="field-hint"
							data-testid="auto-samplerate-hint"
						>
							{view().sampleRateHint}
						</p>
					</div>
				</div>
				<div class="encoder-field-row">
					<label for="output-channels">Channels</label>
					<div class="encoder-field-stack">
						<select
							id="output-channels"
							data-testid="channels-select"
							value={view().channels}
							disabled={view().channelsDisabled}
							onChange={bind('channels')}
						>
							<For each={view().channelOptions}>
								{(option) => <option value={option.value}>{option.label}</option>}
							</For>
						</select>
						<p id="output-channels-effective" class="field-hint" data-testid="auto-channels-hint">
							{view().channelsHint}
						</p>
					</div>
				</div>
			</div>
		</div>
	);
}
