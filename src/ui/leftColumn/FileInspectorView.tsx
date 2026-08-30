import type { JSX } from 'solid-js';
import { toInspectorViewFromInput } from '../../app/inputSession/inspector';
import { useAppRuntime } from '../../app/runtime';
import { companionSummaryForInputIds } from '../remoteSource';

export function FileInspectorView(): JSX.Element {
	const view = useAppRuntime().input.view;
	const inspector = () => toInspectorViewFromInput(view(), companionSummaryForInputIds);

	return (
		<section
			class="left-column-panel file-inspector-panel section-divider file-properties-pinned inspector-footer"
			aria-label="Selected File Properties"
			data-testid="file-inspector-panel"
		>
			<div class="inspector-header">
				<span class="inspector-context" aria-live="polite">
					{inspector().contextVariant === 'empty' ? (
						<span class="context-empty">{inspector().contextText}</span>
					) : (
						<>
							<span class="context-filename" title={inspector().contextText}>
								{inspector().contextText}
							</span>
							{inspector().contextDetail ? (
								<span class="context-position">{inspector().contextDetail}</span>
							) : null}
						</>
					)}
				</span>
			</div>
			<div class="grid grid-cols-2 gap-x-3 gap-y-0.5 text-xs">
				<span class="property-label">Bitrate:</span>
				<span class="property-value">{inspector().bitrateText}</span>
				<span class="property-label">Sample Rate:</span>
				<span class="property-value">{inspector().sampleRateText}</span>
				<span class="property-label">Channels:</span>
				<span class="property-value">{inspector().channelsText}</span>
				<span class="property-label">Codec:</span>
				<span class="property-value">{inspector().codecText}</span>
				<span class="property-label">Decoder:</span>
				<span class="property-value">{inspector().decoderText}</span>
				<span class="property-label">File Size:</span>
				<span class="property-value">{inspector().fileSizeText}</span>
				<span class="property-label">Supplemental:</span>
				<span class="property-value" title={inspector().companionsTitle}>
					{inspector().companionsText}
				</span>
				<span class="property-label">Combined Size:</span>
				<span class="property-value">{inspector().combinedSizeText}</span>
			</div>
		</section>
	);
}
