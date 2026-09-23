import { expect, it } from 'vitest';
import * as input from '.';

it('pins the Input public export strip', () => {
	expect(Object.keys(input).sort()).toEqual(
		[
			'chapterPlansForProcessing',
			'createInputOwner',
			'displayedArtistForFile',
			'displayedTitleForFile',
			'fileListNavigationCommandFromKey',
			'formatAudioProperties',
			'formatFileDetails',
			'interpretFileListKeyDown',
			'nativeDropLooksLikeCoverArt',
			'nativeDropTargetAtPoint',
			'resolveFileListNavigationTarget',
			'toInputView',
			'toInspectorView',
			'toInspectorViewFromInput',
		].sort(),
	);
});
