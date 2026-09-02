import type { SettingsOwner } from '../appSettings';
import type { InputOwner } from '../inputSession';
import type { MetadataOwner } from '../metadataSession';

let boundInput: InputOwner | undefined;
let boundMetadata: MetadataOwner | undefined;
let boundSettings: SettingsOwner | undefined;

export function bindProcessingInput(input: InputOwner | undefined): void {
	boundInput = input;
}

export function bindProcessingMetadata(metadata: MetadataOwner | undefined): void {
	boundMetadata = metadata;
}

export function boundProcessingInput(): InputOwner | undefined {
	return boundInput;
}

export function bindProcessingSettings(settings: SettingsOwner | undefined): void {
	boundSettings = settings;
}

export function boundProcessingMetadata(): MetadataOwner | undefined {
	return boundMetadata;
}

export function boundProcessingSettings(): SettingsOwner | undefined {
	return boundSettings;
}
