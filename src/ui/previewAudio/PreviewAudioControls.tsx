import { createSignal, type JSX } from 'solid-js';
import { useAppRuntime } from '../../app/runtime';
import { SplitButton } from '../foundation';

const PREVIEW_DURATIONS = [15, 30, 45, 60] as const;

export function PreviewAudioControls(props: {
	readonly variant?: 'default' | 'compact';
}): JSX.Element {
	const startProcessing = useAppRuntime().processing.start;
	const [previewDuration, setPreviewDuration] = createSignal(30);

	return (
		<SplitButton
			variant={props.variant}
			testId="preview-audio-controls"
			mainId="preview-button"
			caretId="preview-dropdown-toggle"
			dropdownId="preview-dropdown"
			mainLabel="Preview Audio"
			onMainClick={() => void startProcessing({ previewSeconds: previewDuration() })}
		>
			{({ close }) =>
				PREVIEW_DURATIONS.map((duration) => (
					<SplitButton.Option
						data-duration={String(duration)}
						onClick={() => {
							setPreviewDuration(duration);
							close();
							void startProcessing({ previewSeconds: duration });
						}}
					>
						{duration} seconds
					</SplitButton.Option>
				))
			}
		</SplitButton>
	);
}
