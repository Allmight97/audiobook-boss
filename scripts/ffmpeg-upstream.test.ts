import { readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

import {
	newestAutobuild,
	objectConditions,
	parseCommits,
	parseComponents,
	registeredComponents,
	renderReport,
	type Report,
	unbuiltSources,
	watchedPathspecs,
} from './ffmpeg-upstream';

const sysCrate = path.resolve(import.meta.dirname, '../vendor/ffmpeg-sys-next-9.0.1');
const buildRs = readFileSync(path.join(sysCrate, 'build.rs'), 'utf8');
const provenance = readFileSync(path.join(sysCrate, 'ABB-PROVENANCE.md'), 'utf8');

const codecMakefile = `
OBJS = allcodecs.o
OBJS-$(CONFIG_AAC_ENCODER)             += aacenc.o aaccoder.o \\
                                          aacenctab.o
OBJS-$(CONFIG_H264_DECODER)            += h264dec.o h264_mb.o
OBJS-$(CONFIG_H264_PARSER)             += h264_parser.o h264_mb.o
OBJS-$(CONFIG_HEVC_DECODER)            += hevcdec.o
OBJS-$(CONFIG_HEVCPARSE)               += hevcdec.o
OBJS-$(CONFIG_MPEGAUDIODSP)            += mpegaudiodsp.o
OBJS-$(CONFIG_MP3_DECODER)             += mpegaudiodsp.o
OBJS-$(CONFIG_FRAME_THREAD_ENCODER)    += frame_thread_encoder.o
OBJS-$(CONFIG_AC3_PARSER)              += ac3_parser.o
NEON-OBJS-$(CONFIG_H264_DECODER)       += aarch64/h264dsp_neon.o
X86ASM-OBJS-$(CONFIG_AAC_ENCODER)      += x86/aacencdsp.o
`;

const codecFiles = [
	'libavcodec/aac_parser.c',
	'libavcodec/aacenc.c',
	'libavcodec/aacenc.h',
	'libavcodec/ac3_parser.c',
	'libavcodec/aarch64/h264dsp_neon.S',
	'libavcodec/allcodecs.c',
	'libavcodec/frame_thread_encoder.c',
	'libavcodec/h264_mb.c',
	'libavcodec/h264_mb_template.c',
	'libavcodec/h264dec.c',
	'libavcodec/h264dec.h',
	'libavcodec/hevcdec.c',
	'libavcodec/mpegaudiodsp.c',
	'libavcodec/x86/aacencdsp.asm',
];

function tableComponents(markdown: string): string[] {
	const kinds: Record<string, string> = {
		Demuxers: 'demuxer',
		Muxers: 'muxer',
		Decoders: 'decoder',
		Encoders: 'encoder',
		Parsers: 'parser',
		'Bitstream filters': 'bsf',
		Protocols: 'protocol',
	};
	return markdown
		.split('\n')
		.map((line) => line.split('|').map((cell) => cell.trim()))
		.filter((cells) => cells[1] in kinds)
		.flatMap((cells) =>
			[...cells[2].matchAll(/`([a-z0-9_]+)`/g)].map(([, name]) => `${kinds[cells[1]]} ${name}`),
		);
}

describe('the component lists in build.rs', () => {
	const named = (text: string) => parseComponents(text).map(({ kind, name }) => `${kind} ${name}`);

	it('names every list, the optional and configure-selected ones included', () => {
		const components = named(buildRs);
		for (const expected of [
			'demuxer mov',
			'muxer mp4',
			'decoder mp3float',
			'encoder aac',
			'parser opus',
			'bsf aac_adtstoasc',
			'protocol file',
			'encoder libopus',
			'decoder aac_at',
			'muxer mov',
			'bsf vp9_superframe',
		]) {
			expect(components).toContain(expected);
		}
		expect(components).not.toContain('protocol http');
	});

	it('match the provenance table, apart from what configure selects', () => {
		const selected = /const CONFIGURE_SELECTED[\s\S]*?\n\];/.exec(buildRs)?.[0] ?? '';
		const listed = new Set(
			named(buildRs).filter((component) => !named(selected).includes(component)),
		);
		expect(new Set(tableComponents(provenance))).toEqual(listed);
	});

	it('fails when build.rs lists none', () => {
		expect(() => parseComponents('fn main() {}')).toThrow('no components');
	});
});

const allcodecs = `
extern const FFCodec ff_aac_encoder;
extern const FFCodec ff_h264_decoder;
extern const FFCodec ff_hevc_decoder;
extern const FFCodec ff_mp3_decoder;
`;
const parsers = `
extern const FFCodecParser ff_ac3_parser;
extern const FFCodecParser ff_h264_parser;
`;

describe('sources of unbuilt components', () => {
	const registries = [allcodecs, parsers];
	const built = [
		{ kind: 'encoder', name: 'aac' },
		{ kind: 'parser', name: 'ac3' },
	];

	it('records each condition an object is compiled under, continued lines included', () => {
		const conditions = objectConditions([{ library: 'libavcodec', text: codecMakefile }]);
		expect(conditions.get('libavcodec/aacenctab')).toEqual(new Set(['CONFIG_AAC_ENCODER']));
		expect(conditions.get('libavcodec/allcodecs')).toEqual(new Set(['']));
		expect(conditions.get('libavcodec/h264_mb')).toEqual(
			new Set(['CONFIG_H264_DECODER', 'CONFIG_H264_PARSER']),
		);
		expect(conditions.get('libavcodec/x86/aacencdsp')).toEqual(new Set(['CONFIG_AAC_ENCODER']));
	});

	it('name the components FFmpeg registers', () => {
		expect(registeredComponents(registries)).toEqual(
			new Set([
				'CONFIG_AAC_ENCODER',
				'CONFIG_H264_DECODER',
				'CONFIG_HEVC_DECODER',
				'CONFIG_MP3_DECODER',
				'CONFIG_AC3_PARSER',
				'CONFIG_H264_PARSER',
			]),
		);
	});

	it('leave out only sources every line compiles for an unbuilt component', () => {
		expect(
			unbuiltSources(
				built,
				registries,
				[{ library: 'libavcodec', text: codecMakefile }],
				codecFiles,
			),
		).toEqual([
			'libavcodec/aarch64/h264dsp_neon.S',
			'libavcodec/h264_mb.c',
			'libavcodec/h264dec.c',
		]);
	});

	it('keep a source another Makefile compiles for a built component', () => {
		const master = 'OBJS-$(CONFIG_AAC_ENCODER) += h264dec.o\n';
		expect(
			unbuiltSources(
				built,
				registries,
				[
					{ library: 'libavcodec', text: codecMakefile },
					{ library: 'libavcodec', text: master },
				],
				codecFiles,
			),
		).not.toContain('libavcodec/h264dec.c');
	});

	it('watch the libraries and configure whole, minus those sources', () => {
		expect(watchedPathspecs(['libavcodec/h264dec.c'])).toEqual([
			'configure',
			'libavcodec',
			'libavformat',
			'libavutil',
			'libswresample',
			':(exclude)libavcodec/h264dec.c',
		]);
	});
});

describe('commits', () => {
	it('splits tab-separated log lines and keeps tabs in a subject', () => {
		expect(
			parseCommits('abc123\t2026-11-10\tavcodec/aac: fix\nfff000\t2026-11-09\ta\tb\n'),
		).toEqual([
			{ hash: 'abc123', date: '2026-11-10', subject: 'avcodec/aac: fix' },
			{ hash: 'fff000', date: '2026-11-09', subject: 'a\tb' },
		]);
		expect(parseCommits('')).toEqual([]);
	});
});

describe('the BtbN autobuild', () => {
	it('skips the moving latest tag and names the master asset with its commit', () => {
		expect(
			newestAutobuild([
				{ tag: 'latest', assets: ['ffmpeg-N-1-g1111111-linux64-gpl.tar.xz'] },
				{
					tag: 'autobuild-2026-11-10-12-39',
					assets: [
						'ffmpeg-N-128728-g49a3b1f259-linux64-gpl-shared.tar.xz',
						'ffmpeg-N-128728-g49a3b1f259-linux64-gpl.tar.xz',
						'ffmpeg-n9.0.2-48-gd4070a14ce-linux64-gpl-9.0.tar.xz',
					],
				},
			]),
		).toEqual({
			tag: 'autobuild-2026-11-10-12-39',
			asset: 'ffmpeg-N-128728-g49a3b1f259-linux64-gpl.tar.xz',
			builtFrom: '49a3b1f259',
		});
	});

	it('finds none when no release has a master asset', () => {
		expect(newestAutobuild([{ tag: 'latest', assets: [] }])).toBeNull();
	});
});

describe('the report', () => {
	const base: Report = {
		pin: '49a3b1f259e1c34033bf6cca4091a4cb07ebf90d',
		head: '1111111111111111111111111111111111111111',
		commits: [],
		autobuild: { found: null },
	};

	it('says so when nothing relevant changed', () => {
		expect(renderReport(base)).toContain('no relevant upstream changes');
	});

	it('lists each commit and escapes a table bar in a subject', () => {
		const text = renderReport({
			...base,
			commits: [{ hash: 'abcdef0123456789', date: '2026-11-10', subject: 'aac | fix' }],
			autobuild: {
				found: {
					tag: 'autobuild-2026-11-10-12-39',
					asset: 'ffmpeg-N-1-g1111111-linux64-gpl.tar.xz',
					builtFrom: '1111111',
				},
			},
		});
		expect(text).toContain('| `abcdef0123` | 2026-11-10 | aac \\| fix |');
		expect(text).not.toContain('no relevant upstream changes');
		expect(text).toContain('built from master head');
	});

	it('keeps the commits when the autobuilds could not be read', () => {
		const text = renderReport({
			...base,
			commits: [{ hash: 'abcdef0123456789', date: '2026-11-10', subject: 'aac: fix' }],
			autobuild: { error: 'HTTP 502' },
		});
		expect(text).toContain('| `abcdef0123` | 2026-11-10 | aac: fix |');
		expect(text).toContain('autobuild: could not read (HTTP 502)');
	});
});
