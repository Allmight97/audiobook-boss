import type { AudioFile } from '../../types/audio';
import type { InputView } from '../inputSession';

export function validTitlesFromInput(view: InputView): AudioFile[] {
	return view.files.filter((file) => file.isValid);
}
