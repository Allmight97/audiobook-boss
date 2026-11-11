import { readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

import {
	makefileIncludes,
	newestAutobuild,
	parseCommits,
	parseComponents,
	patchedFiles,
	renderReport,
	type Report,
	sourcesOf,
	watchedPatterns,
} from './ffmpeg-upstream';

const sysCrate = path.resolve(import.meta.dirname, '../vendor/ffmpeg-sys-next-9.0.1');
const buildRs = readFileSync(path.join(sysCrate, 'build.rs'), 'utf8');

const codecMakefile = `
OBJS-$(CONFIG_AAC_ENCODER)             += aacenc.o aaccoder.o \\
                                          aacenctab.o
OBJS-$(CONFIG_PCM_S16LE_DECODER)       += pcm.o
OBJS-$(CONFIG_H264_DECODER)            += h264dec.o
OBJS-$(CONFIG_AAC_PARSER)              += aac_parser.o
OBJS-$(CONFIG_MP3FLOAT_DECODER)        += mpegaudiodec_float.o
NEON-OBJS-$(CONFIG_AAC_ENCODER)        += aarch64/aacencdsp_neon.o
include $(SRC_PATH)/libavcodec/aac/Makefile
include $(SRC_PATH)/libavcodec/$(ARCH)/Makefile
`;

describe('the component list in build.rs', () => {
	it('names each group ABB builds, and the optional encoders and decoders', () => {
		const components = parseComponents(buildRs);
		for (const expected of [
			{ kind: 'demuxer', name: 'mov' },
			{ kind: 'muxer', name: 'ipod' },
			{ kind: 'decoder', name: 'mp3float' },
			{ kind: 'encoder', name: 'aac' },
			{ kind: 'parser', name: 'opus' },
			{ kind: 'bsf', name: 'aac_adtstoasc' },
			{ kind: 'protocol', name: 'file' },
			{ kind: 'encoder', name: 'libopus' },
			{ kind: 'decoder', name: 'aac_at' },
		]) {
			expect(components).toContainEqual(expected);
		}
		expect(components).not.toContainEqual({ kind: 'protocol', name: 'http' });
	});

	it('fails when build.rs has no list', () => {
		expect(() => parseComponents('fn main() {}')).toThrow('ABB_COMPONENTS');
	});
});

describe('sources of the enabled components', () => {
	it('reads continued Makefile lines, arch variants and templates, and skips other components', () => {
		const sources = sourcesOf(
			[
				{ kind: 'encoder', name: 'aac' },
				{ kind: 'decoder', name: 'pcm_s16le' },
				{ kind: 'decoder', name: 'mp3float' },
				{ kind: 'parser', name: 'aac' },
			],
			[{ library: 'libavcodec', text: codecMakefile }],
		);
		expect(sources).toEqual([
			'libavcodec/aac_parser*',
			'libavcodec/aaccoder*',
			'libavcodec/aacenc*',
			'libavcodec/aacenctab*',
			'libavcodec/aarch64/aacencdsp_neon*',
			'libavcodec/mpegaudiodec_float*',
			'libavcodec/pcm*',
		]);
	});

	it('keeps a component to its own library', () => {
		const sources = sourcesOf(
			[{ kind: 'muxer', name: 'aac' }],
			[{ library: 'libavcodec', text: codecMakefile }],
		);
		expect(sources).toEqual([]);
	});

	it('lists the literal sub-Makefiles to read', () => {
		expect(makefileIncludes(codecMakefile)).toEqual(['libavcodec/aac/Makefile']);
	});
});

describe('the chapter patch', () => {
	it('names the files it touches', () => {
		const patch = readFileSync(path.join(sysCrate, 'patches/mov-chapter-start.patch'), 'utf8');
		expect(patchedFiles(patch)).toEqual(['libavformat/movenc.c']);
	});
});

describe('the watched paths', () => {
	it('add the fixed list and the chapter patch to the component sources, once each', () => {
		const patch = readFileSync(path.join(sysCrate, 'patches/mov-chapter-start.patch'), 'utf8');
		const watched = watchedPatterns(buildRs, patch, [
			{ library: 'libavcodec', text: codecMakefile },
		]);
		expect(watched).toContain('libavcodec/aacenc*');
		expect(watched).toContain('libavcodec/mpegaudio*');
		expect(watched).toContain('libavcodec/pcm*');
		expect(watched).toContain('libavformat/movenc.c');
		expect(watched).toContain('libswresample');
		expect(new Set(watched).size).toBe(watched.length);
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
	const assets: Record<string, string[]> = {
		'autobuild-2026-11-10-12-39': [
			'ffmpeg-N-128728-g49a3b1f259-linux64-gpl-shared.tar.xz',
			'ffmpeg-N-128728-g49a3b1f259-linux64-gpl.tar.xz',
			'ffmpeg-n9.0.2-48-gd4070a14ce-linux64-gpl-9.0.tar.xz',
		],
	};

	it('skips the moving latest tag and names the master asset with its commit', () => {
		expect(
			newestAutobuild(['latest', 'autobuild-2026-11-10-12-39'], (tag) => assets[tag] ?? []),
		).toEqual({
			tag: 'autobuild-2026-11-10-12-39',
			asset: 'ffmpeg-N-128728-g49a3b1f259-linux64-gpl.tar.xz',
			builtFrom: '49a3b1f259',
		});
	});

	it('finds none when no release has a master asset', () => {
		expect(newestAutobuild(['latest'], () => [])).toBeNull();
	});
});

describe('the report', () => {
	const base: Report = {
		pin: '49a3b1f259e1c34033bf6cca4091a4cb07ebf90d',
		head: '1111111111111111111111111111111111111111',
		commits: [],
		autobuild: null,
		autobuildMatchesHead: false,
	};

	it('says so when nothing relevant changed', () => {
		expect(renderReport(base)).toContain('no relevant upstream changes');
	});

	it('lists each commit and escapes a table bar in a subject', () => {
		const text = renderReport({
			...base,
			commits: [{ hash: 'abcdef0123456789', date: '2026-11-10', subject: 'aac | fix' }],
			autobuild: {
				tag: 'autobuild-2026-11-10-12-39',
				asset: 'ffmpeg-N-1-g1111111-linux64-gpl.tar.xz',
				builtFrom: '1111111',
			},
			autobuildMatchesHead: true,
		});
		expect(text).toContain('| `abcdef0123` | 2026-11-10 | aac \\| fix |');
		expect(text).not.toContain('no relevant upstream changes');
		expect(text).toContain('built from master head');
	});
});
