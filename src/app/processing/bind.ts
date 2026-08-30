import type { EncodingRequestConfig } from '../../types/audio';
import type { SettingsOwner } from '../appSettings/owner';
import type { InputOwner } from '../inputSession/owner';
import type { MetadataOwner } from '../metadataSession/owner';

let boundInput: InputOwner | undefined;
let boundMetadata: MetadataOwner | undefined;
let boundSettings: SettingsOwner | undefined;
let boundEncoding: (() => EncodingRequestConfig) | undefined;

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

export function bindProcessingEncoding(read: (() => EncodingRequestConfig) | undefined): void {
	boundEncoding = read;
}

export function boundProcessingEncoding(): (() => EncodingRequestConfig) | undefined {
	return boundEncoding;
}
