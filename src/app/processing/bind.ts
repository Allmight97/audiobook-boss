import type { EncodingRequestConfig } from '../../types/audio';
import type { SettingsOwner } from '../appSettings';
import type { InputOwner } from '../inputSession';
import type { MetadataOwner } from '../metadataSession';
import type { RemoteSourceOwner } from '../remoteSource';

let boundInput: InputOwner | undefined;
let boundMetadata: MetadataOwner | undefined;
let boundSettings: SettingsOwner | undefined;
let boundEncoding: (() => EncodingRequestConfig) | undefined;
let boundRemoteSource: RemoteSourceOwner | undefined;

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

export function bindProcessingRemoteSource(remoteSource: RemoteSourceOwner | undefined): void {
	boundRemoteSource = remoteSource;
}

export function boundProcessingRemoteSource(): RemoteSourceOwner | undefined {
	return boundRemoteSource;
}
