import { beforeEach, describe, expect, it } from 'vitest';

import * as metadataForm from '..';

const EXPECTED_METADATA_FORM_EXPORTS = [
	'applyMetadataFormValidationWarnings',
	'applyMetadataToForm',
	'hasDirtyMetadataFields',
	'onMetadataFormActionSelectChange',
	'onMetadataFormFieldInput',
	'populateMetadataFormMulti',
	'populateMetadataFormSingle',
	'readMetadataForm',
	'readMetadataFormRevision',
	'readMetadataFormViewSnapshot',
	'resetDirtyState',
	'setMetadataFormMode',
] as const;

describe('Metadata Form runtime public API contract', () => {
	beforeEach(() => {
		metadataForm.populateMetadataFormSingle({});
	});

	it('pins the metadata form public export strip', () => {
		expect(Object.keys(metadataForm).sort()).toEqual([...EXPECTED_METADATA_FORM_EXPORTS].sort());
	});

	it('reads view state through the public snapshot accessor', () => {
		metadataForm.setMetadataFormMode('multi', 3);

		expect(metadataForm.readMetadataFormViewSnapshot()).toEqual({
			mode: 'multi',
			selectionCount: 3,
		});
	});
});
