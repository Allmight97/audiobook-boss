import type {
	AudioChoice,
	AudioChoiceFacts,
	AudioChoiceView,
	BitrateMode,
	ConcurrencyPreference,
	EncoderDefaults,
	EncoderSettings,
	EncoderSettingsCapabilities,
	FieldSnapshot,
	FormatOption,
	FrontendAttachment,
	LookupSnapshot,
	MetadataSnapshot,
	OutputDefaults,
	OutputNamingConfig,
	OutputPreview,
	OutputSnapshot,
	RemoteDraftStatus,
	RemoteUiSnapshot,
	SampleRateConfig,
	SessionUpdate,
	SettingsSnapshot,
	TitlesSnapshot,
} from '../generated/tauri';
import sessionFresh from '../generated/fixtures/session-fresh.json';
import settingsFresh from '../generated/fixtures/settings-fresh.json';

function oneOf<const Allowed extends readonly string[]>(
	value: string,
	allowed: Allowed,
	label: string,
): Allowed[number] {
	for (const candidate of allowed) {
		if (value === candidate) return candidate;
	}
	throw new Error(`recorded fresh ${label} is ${value}`);
}

function bitrateMode(value: { mode: string }): BitrateMode {
	const mode = oneOf(
		value.mode,
		['cbr', 'cvbr', 'abr', 'vbr', 'vbr_target'] as const,
		'bitrate mode',
	);
	if (mode === 'vbr') {
		if (!('value' in value) || typeof value.value !== 'number') {
			throw new Error('recorded fresh vbr bitrate mode has no value');
		}
		return { mode, value: value.value };
	}
	return { mode };
}

function sampleRate(value: string | { explicit: number }): SampleRateConfig {
	if (typeof value === 'string') return oneOf(value, ['auto'] as const, 'sample rate');
	return value;
}

function concurrency(value: { mode: string }): ConcurrencyPreference {
	const mode = oneOf(value.mode, ['auto', 'fixed'] as const, 'concurrency');
	if (mode === 'auto') return { mode };
	if (!('value' in value) || typeof value.value !== 'number') {
		throw new Error('recorded fresh fixed concurrency has no value');
	}
	return { mode, value: value.value };
}

function encoderSettings(value: {
	bitrateKbps: number;
	bitrateMode: { mode: string };
	channels: string;
	encoderType: string;
	faacProfile: string;
	nativeAacSpeed?: number;
}): EncoderSettings {
	return {
		bitrateKbps: value.bitrateKbps,
		bitrateMode: bitrateMode(value.bitrateMode),
		channels: oneOf(value.channels, ['auto', 'mono', 'stereo'] as const, 'channels'),
		encoderType: oneOf(
			value.encoderType,
			['auto', 'aac_at', 'native_aac', 'faac', 'opus'] as const,
			'encoder',
		),
		faacProfile: oneOf(value.faacProfile, ['auto', 'aac_lc', 'he_aac_v1'] as const, 'faac profile'),
		nativeAacSpeed: value.nativeAacSpeed,
	};
}

function audioChoice(value: typeof sessionFresh.audio.defaults.choice): AudioChoice {
	return {
		...value,
		channels: oneOf(value.channels, ['auto', 'mono', 'stereo'] as const, 'channels'),
		encoder: oneOf(
			value.encoder,
			['auto', 'aac_at', 'native_aac', 'faac', 'opus'] as const,
			'encoder',
		),
		faacProfile: oneOf(value.faacProfile, ['auto', 'aac_lc', 'he_aac_v1'] as const, 'faac profile'),
		faacRateControl: oneOf(value.faacRateControl, ['abr', 'vbr'] as const, 'faac rate control'),
		format: oneOf(value.format, ['m4b', 'm4aOpus', 'mkaOpus'] as const, 'format'),
		intent: oneOf(value.intent, ['auto', 'preserve', 'encode'] as const, 'intent'),
		sampleRate: sampleRate(value.sampleRate),
	};
}

function formatOption(value: { codecFamily: string; format: string }): FormatOption {
	return {
		codecFamily: oneOf(value.codecFamily, ['aac', 'opus'] as const, 'codec family'),
		format: oneOf(value.format, ['m4b', 'm4aOpus', 'mkaOpus'] as const, 'format'),
	};
}

function audioFacts(value: typeof sessionFresh.audio.defaults.facts): AudioChoiceFacts {
	return {
		...value,
		formatOptions: value.formatOptions.map((option) => formatOption(option)),
		codecFamily: oneOf(value.codecFamily, ['aac', 'opus'] as const, 'codec family'),
		intentOptions: value.intentOptions.map((intent) =>
			oneOf(intent, ['auto', 'preserve', 'encode'] as const, 'intent'),
		),
		channelOptions: value.channelOptions.map((option) =>
			oneOf(option, ['auto', 'mono', 'stereo'] as const, 'channels'),
		),
		effectiveEncoder: oneOf(
			value.effectiveEncoder,
			['auto', 'aac_at', 'native_aac', 'faac', 'opus'] as const,
			'encoder',
		),
		encoderOptions: value.encoderOptions.map((option) => ({
			available: option.available,
			encoder: oneOf(
				option.encoder,
				['auto', 'aac_at', 'native_aac', 'faac', 'opus'] as const,
				'encoder',
			),
		})),
		bitrateMode: bitrateMode(value.bitrateMode),
		allowedModes: value.allowedModes.map((mode) =>
			oneOf(mode, ['cbr', 'cvbr', 'abr', 'vbr'] as const, 'bitrate mode'),
		),
		faacProfiles: value.faacProfiles.map((profile) =>
			oneOf(profile, ['auto', 'aac_lc', 'he_aac_v1'] as const, 'faac profile'),
		),
	};
}

function audioRequest(
	value: typeof sessionFresh.audio.defaults.request,
): AudioChoiceView['request'] {
	const format = oneOf(value.format, ['m4b', 'm4aOpus', 'mkaOpus'] as const, 'format');
	return {
		format,
		intent: oneOf(value.intent, ['auto', 'preserve', 'encode'] as const, 'intent'),
		sampleRate: sampleRate(value.sampleRate),
		settings: encoderSettings(value.settings),
	};
}

function audioDefaults(value: typeof sessionFresh.audio.defaults): AudioChoiceView {
	return {
		choice: audioChoice(value.choice),
		facts: audioFacts(value.facts),
		request: audioRequest(value.request),
	};
}

function capabilities(value: typeof sessionFresh.audio.capabilities): EncoderSettingsCapabilities {
	return {
		...value,
		channelOptions: value.channelOptions.map((option) =>
			oneOf(option, ['auto', 'mono', 'stereo'] as const, 'channels'),
		),
	};
}

function lookup(value: typeof sessionFresh.lookup): LookupSnapshot {
	return {
		...value,
		applyMode: oneOf(value.applyMode, ['current', 'queue'] as const, 'lookup apply mode'),
		source: oneOf(value.source, ['auto', 'audnexus', 'openlibrary'] as const, 'lookup source'),
	};
}

function metadataField(value: (typeof sessionFresh.metadata.form.fields)[number]): FieldSnapshot {
	return {
		...value,
		field: oneOf(
			value.field,
			[
				'title',
				'date',
				'author',
				'narrator',
				'series',
				'seriesPart',
				'subseries',
				'subseriesPart',
				'genre',
				'description',
			] as const,
			'metadata field',
		),
		action: oneOf(value.action, ['keep', 'blank'] as const, 'field action'),
	};
}

function metadata(value: typeof sessionFresh.metadata): MetadataSnapshot {
	return {
		...value,
		form: {
			...value.form,
			mode: oneOf(value.form.mode, ['single', 'multi'] as const, 'form mode'),
			fields: value.form.fields.map((field) => metadataField(field)),
		},
	};
}

function outputPreview(value: typeof sessionFresh.output.preview): OutputPreview {
	const kind = oneOf(
		value.kind,
		['noDirectory', 'noTitle', 'path', 'unavailable'] as const,
		'output preview',
	);
	if (kind === 'noDirectory' || kind === 'noTitle') return { kind };
	if (kind === 'path' && 'path' in value && typeof value.path === 'string')
		return { kind, path: value.path };
	if (kind === 'unavailable' && 'message' in value && typeof value.message === 'string') {
		return { kind, message: value.message };
	}
	throw new Error(`recorded fresh output preview is ${kind}`);
}

function outputNaming(value: {
	preset: string;
	includeYear: boolean;
	customTemplate: string | null;
}): OutputNamingConfig {
	return {
		includeYear: value.includeYear,
		customTemplate: value.customTemplate,
		preset: oneOf(value.preset, ['absDefault', 'customTemplate'] as const, 'naming preset'),
	};
}

function output(value: typeof sessionFresh.output): OutputSnapshot {
	return {
		...value,
		preset: oneOf(value.preset, ['absDefault', 'customTemplate'] as const, 'naming preset'),
		naming: outputNaming(value.naming),
		preview: outputPreview(value.preview),
	};
}

function draftStatus(value: { kind: string }): RemoteDraftStatus {
	const kind = oneOf(value.kind, ['idle', 'running', 'succeeded'] as const, 'draft status');
	return { kind };
}

function remote(value: typeof sessionFresh.remote): RemoteUiSnapshot {
	return {
		...value,
		lane: oneOf(value.lane, ['audible', 'indexer'] as const, 'provider'),
		providers: value.providers.map((provider) => ({
			...provider,
			providerId: oneOf(provider.providerId, ['audible', 'indexer'] as const, 'provider'),
		})),
		connection: {
			...value.connection,
			save: draftStatus(value.connection.save),
			test: draftStatus(value.connection.test),
		},
	};
}

function titles(value: typeof sessionFresh.titles): TitlesSnapshot {
	return {
		...value,
		sortDirection: oneOf(
			value.sortDirection,
			['none', 'ascending', 'descending'] as const,
			'sort direction',
		),
	};
}

function session(value: typeof sessionFresh): SessionUpdate {
	return {
		...value,
		audio: {
			...value.audio,
			capabilities: capabilities(value.audio.capabilities),
			defaults: audioDefaults(value.audio.defaults),
		},
		lookup: lookup(value.lookup),
		metadata: metadata(value.metadata),
		output: output(value.output),
		remote: remote(value.remote),
		titles: titles(value.titles),
	};
}

function encoderDefaults(value: typeof settingsFresh.settings.encoderDefaults): EncoderDefaults {
	return {
		format: oneOf(value.format, ['m4b', 'm4aOpus', 'mkaOpus'] as const, 'format'),
		intent: oneOf(value.intent, ['auto', 'preserve', 'encode'] as const, 'intent'),
		sampleRate: sampleRate(value.sampleRate),
		settings: encoderSettings(value.settings),
	};
}

function outputDefaults(value: typeof settingsFresh.settings.outputDefaults): OutputDefaults {
	return {
		outputDirectory: value.outputDirectory,
		outputNaming: outputNaming(value.outputNaming),
	};
}

function settings(value: typeof settingsFresh): SettingsSnapshot {
	return {
		...value,
		concurrency: {
			...value.concurrency,
			preference: concurrency(value.concurrency.preference),
		},
		settings: {
			...value.settings,
			aacDecoder: oneOf(value.settings.aacDecoder, ['auto', 'faad'] as const, 'aac decoder'),
			defaultAcquisitionLane: oneOf(
				value.settings.defaultAcquisitionLane,
				['audible', 'indexer'] as const,
				'acquisition lane',
			),
			encoderDefaults: encoderDefaults(value.settings.encoderDefaults),
			maxConcurrentJobs: concurrency(value.settings.maxConcurrentJobs),
			outputDefaults: outputDefaults(value.settings.outputDefaults),
			startupBehavior: oneOf(
				value.settings.startupBehavior,
				['rememberLastState', 'pinnedDefaults'] as const,
				'startup behavior',
			),
		},
	};
}

export const recordedFresh: FrontendAttachment = {
	client: 1,
	session: session(sessionFresh),
	settings: settings(settingsFresh),
	workOperations: { revision: 0, order: [], operations: [], concurrencyLocked: false },
};
