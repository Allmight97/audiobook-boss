import type { JSX } from '@solidjs/web';
import { useAppRuntime } from '../../app/runtime';
import './jobControls.css';

export function GroupTitlesButton(): JSX.Element {
	const input = useAppRuntime().input;
	return (
		<button
			type="button"
			class="group-titles-button"
			disabled={input.view().orderLocked || input.view().selectedIndices.length < 2}
			title="Select two or more titles to group their files into one audiobook."
			onClick={() => void input.groupSelected()}
		>
			Group as one title
		</button>
	);
}
