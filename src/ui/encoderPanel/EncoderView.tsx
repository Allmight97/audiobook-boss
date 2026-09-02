import { For } from 'solid-js';
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
			runtime.encoding.select(field, (event.currentTarget as HTMLSelectElement).value);
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
					<label for="output-quality" id="quality-bitrate-label">
						{view().qualityBitrateLabel}
					</label>
					<div class="encoder-field-stack">
						<select
							id="output-quality"
							hidden={!view().showQuality}
							data-testid="quality-select"
							value={view().quality}
							onChange={bind('quality')}
						>
							<For each={view().qualityOptions}>
								{(option) => <option value={option.value}>{option.label}</option>}
							</For>
						</select>
						<select
							id="output-bitrate"
							hidden={view().showQuality}
							data-testid="bitrate-select"
							value={view().bitrate}
							onChange={bind('bitrate')}
						>
							<For each={view().bitrateOptions}>
								{(option) => <option value={option.value}>{option.label}</option>}
							</For>
						</select>
						<p id="estimated-bitrate" class="field-hint" data-testid="estimated-bitrate">
							{view().estimatedBitrateText}
						</p>
					</div>
				</div>
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
