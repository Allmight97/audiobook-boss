import { render } from 'solid-js/web';
import { FileListIsland, type FileListIslandProps } from './FileListIsland';

export function mountFileListIsland(el: HTMLElement, props: FileListIslandProps = {}): () => void {
	return render(() => <FileListIsland {...props} />, el);
}
