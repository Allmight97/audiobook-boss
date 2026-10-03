import { expect, it } from 'vitest';
import * as input from '.';

it('pins the Input public export strip', () => {
	expect(Object.keys(input).sort()).toEqual(
		[
			'createInputOwner',
			'displayedArtistForFile',
			'displayedTitleForFile',
			'fileListNavigationCommandFromKey',
			'formatAudioProperties',
			'formatFileDetails',
			'interpretFileListKeyDown',
			'nativeDropTargetAtPoint',
			'resolveFileListNavigationTarget',
			'toInputView',
			'toInspectorViewFromInput',
		].sort(),
	);
});
