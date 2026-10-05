import type {
	MetadataSource as GeneratedMetadataSource,
	OnlineMetadataResult as GeneratedOnlineMetadataResult,
} from '../lib/generated/tauri';
import type { NullToOptionalDeep } from './ipc';

export type MetadataSource = GeneratedMetadataSource;

export type OnlineMetadataResult = NullToOptionalDeep<GeneratedOnlineMetadataResult>;
