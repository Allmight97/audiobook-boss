import { createSignal, For, onCleanup, onMount, type JSX } from 'solid-js';
import { estimatedSizeTextAtom } from '../../app/outputPlan';
import { useAtomSet, useAtomValue } from '../../app/runtime/solid';
import {
	handleBitrateModeChange,
	handleBitrateValueChange,
	handleChannelsSelectionChange,
	handleFlavorChange,
	handleQualityValueChange,
	handleSampleRateSelectionChange,
	initializeEncoderPanelLogic,
} from '../encoderPanel/logic';
import { encodingRequestConfigAtom, publishEncodingRequestConfig } from '../encoderPanel/requestConfig';
import { encoderPanelState, subscribeEncoderPanel } from '../encoderPanel/state.svelte';
import {
	bitrateModeLabel,
	channelLabel,
	channelsDetailText,
	encoderLabel,
	encoderOptionDisabled,
	qualityLabel,
	sampleRateDetailText,
	sampleRateLabel,
} from '../encoderPanel/view';
import './encoderView.css';

export function EncoderView(): JSX.Element {
	const [revision, setRevision] = createSignal(0);
	const estimatedSizeText = useAtomValue(() => estimatedSizeTextAtom);
	const setEncodingRequestConfig = useAtomSet(() => encodingRequestConfigAtom);
	const state = () => {
		revision();
		return encoderPanelState;
	};

	function bump(): void {
		setRevision((value) => value + 1);
		publishEncodingRequestConfig(setEncodingRequestConfig);
	}

	onMount(() => {
		onCleanup(subscribeEncoderPanel(bump));
		initializeEncoderPanelLogic();
		bump();
	});

	function wrap(handler: (event: Event) => void) {
		return (event: Event) => {
			handler(event);
			bump();
		};
	}

	function optionLabel(encoderOption: string): string {
		state();
		return encoderLabel(encoderOption);
	}

	function sampleHint(): string {
		state();
		return sampleRateDetailText();
	}

	function channelHint(): string {
		state();
		return channelsDetailText();
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
							value={state().flavor}
							disabled={state().encoderOptions.length === 0}
							onChange={wrap(handleFlavorChange)}
						>
							{state().encoderOptions.length === 0 ? (
								<option value="auto">Loading…</option>
							) : (
								<For each={state().encoderOptions}>
									{(encoderOption) => (
										<option value={encoderOption} disabled={encoderOptionDisabled(encoderOption)}>
											{optionLabel(encoderOption)}
										</option>
									)}
								</For>
							)}
						</select>
						<p
							id="encoder-availability-hint"
							class="field-hint"
							data-testid="encoder-availability-hint"
						>
							{state().availabilityHint}
						</p>
					</div>
				</div>
				<div class="encoder-field-row">
					<span class="label">Profile</span>
					<div class="profile-display profile-display-workbench" data-testid="profile-display">
						<span id="encoder-profile-display">{state().profileDisplay}</span>
						<span class="readonly-badge">read-only</span>
					</div>
				</div>
				<div class="encoder-field-row">
					<label for="adv-bitrate-mode">Bitrate Mode</label>
					<select
						id="adv-bitrate-mode"
						data-testid="bitrate-mode-select"
						value={state().bitrateModeSelection}
						disabled={state().bitrateModeOptions.length === 0}
						onChange={wrap(handleBitrateModeChange)}
					>
						<For each={state().bitrateModeOptions}>
							{(bitrateModeOption) => (
								<option
									value={bitrateModeOption}
									disabled={!state().bitrateModeAvailability[bitrateModeOption]}
								>
									{bitrateModeLabel(bitrateModeOption)}
								</option>
							)}
						</For>
					</select>
				</div>
				<div class="encoder-field-row">
					<label for="output-quality" id="quality-bitrate-label">
						{state().qualityBitrateLabel}
					</label>
					<div class="encoder-field-stack">
						<select
							id="output-quality"
							classList={{ hidden: !state().showQuality }}
							data-testid="quality-select"
							value={state().qualityValue}
							onChange={wrap(handleQualityValueChange)}
						>
							<For each={state().qualityOptions}>
								{(qualityOption) => (
									<option value={qualityOption}>{qualityLabel(qualityOption)}</option>
								)}
							</For>
						</select>
						<select
							id="output-bitrate"
							classList={{ hidden: state().showQuality }}
							data-testid="bitrate-select"
							value={state().bitrateValue}
							onChange={wrap(handleBitrateValueChange)}
						>
							<For each={state().bitrateOptions}>
								{(bitrateOption) => <option value={bitrateOption}>{bitrateOption} kbps</option>}
							</For>
						</select>
						<p id="estimated-bitrate" class="field-hint" data-testid="estimated-bitrate">
							{state().estimatedBitrateText}
						</p>
					</div>
				</div>
				<div class="encoder-field-row">
					<label for="output-samplerate">Sample Rate</label>
					<div class="encoder-field-stack">
						<select
							id="output-samplerate"
							data-testid="samplerate-select"
							value={state().sampleRateSelection}
							disabled={state().sampleRateOptions.length === 0}
							onChange={wrap(handleSampleRateSelectionChange)}
						>
							<For each={state().sampleRateOptions}>
								{(sampleRateOption) => (
									<option value={sampleRateOption}>{sampleRateLabel(sampleRateOption)}</option>
								)}
							</For>
						</select>
						<p
							id="output-samplerate-effective"
							class="field-hint"
							data-testid="auto-samplerate-hint"
						>
							{sampleHint()}
						</p>
					</div>
				</div>
				<div class="encoder-field-row">
					<label for="output-channels">Channels</label>
					<div class="encoder-field-stack">
						<select
							id="output-channels"
							data-testid="channels-select"
							value={state().channelsSelection}
							disabled={state().channelOptions.length === 0}
							onChange={wrap(handleChannelsSelectionChange)}
						>
							<For each={state().channelOptions}>
								{(channelOption) => (
									<option value={channelOption}>{channelLabel(channelOption)}</option>
								)}
							</For>
						</select>
						<p id="output-channels-effective" class="field-hint" data-testid="auto-channels-hint">
							{channelHint()}
						</p>
					</div>
				</div>
			</div>
		</div>
	);
}
