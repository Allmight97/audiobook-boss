import { readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

import {
	decideIssueAction,
	type Gh,
	ISSUE_TITLE,
	issueBody,
	objectConditions,
	parseCommits,
	parseComponents,
	registeredComponents,
	renderReport,
	type Report,
	syncIssue,
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

describe('the report', () => {
	const base: Report = {
		pin: '49a3b1f259e1c34033bf6cca4091a4cb07ebf90d',
		head: '1111111111111111111111111111111111111111',
		commits: [],
	};

	it('says so when nothing relevant changed', () => {
		expect(renderReport(base)).toContain('no relevant upstream changes');
	});

	it('lists each commit and escapes a table bar in a subject', () => {
		const text = renderReport({
			...base,
			commits: [{ hash: 'abcdef0123456789', date: '2026-11-10', subject: 'aac | fix' }],
		});
		expect(text).toContain('| `abcdef0123` | 2026-11-10 | aac \\| fix |');
		expect(text).not.toContain('no relevant upstream changes');
		expect(text).not.toContain('BtbN');
	});
});

describe('the issue', () => {
	const checkedAt = new Date('2026-11-11T17:37:45Z');
	const commit = { hash: 'abcdef0123456789', date: '2026-11-10', subject: 'aac: fix' };
	const report: Report = {
		pin: '49a3b1f259e1c34033bf6cca4091a4cb07ebf90d',
		head: '1111111111111111111111111111111111111111',
		commits: [commit],
	};
	const open = (body: string) => ({ number: 634, title: ISSUE_TITLE, body });

	it('opens with the check time and pin, then the next action, then the report', () => {
		const lines = issueBody(report, checkedAt).split('\n');
		expect(lines[0]).toBe('Checked 2026-11-11 17:37 UTC against pin 49a3b1f259.');
		expect(lines[2]).toMatch(/^Next action: decide whether to move the pin/);
		expect(lines[4]).toBe('# FFmpeg upstream watch');
	});

	it('creates the issue when commits exist and none is open', () => {
		expect(decideIssueAction(report, undefined, checkedAt)).toEqual({
			kind: 'create',
			body: issueBody(report, checkedAt),
		});
	});

	it('leaves the issue alone when the pin and commits match, even if the head moved', () => {
		const older = issueBody(
			{ ...report, head: '2222222222222222222222222222222222222222' },
			new Date('2026-11-09T00:00:00Z'),
		);
		expect(decideIssueAction(report, open(older), checkedAt)).toEqual({
			kind: 'unchanged',
			number: 634,
		});
	});

	it('updates the issue when a commit is added or the pin moves', () => {
		const older = issueBody(report, checkedAt);
		const more = { ...report, commits: [{ ...commit, hash: 'fff0001112223334' }, commit] };
		expect(decideIssueAction(more, open(older), checkedAt).kind).toBe('update');
		const moved = { ...report, pin: '9bee1e603c000000000000000000000000000000' };
		expect(decideIssueAction(moved, open(older), checkedAt).kind).toBe('update');
	});

	it('closes an open issue when no commits remain, and does nothing when none is open', () => {
		const clean = { ...report, commits: [] };
		expect(decideIssueAction(clean, open('x'), checkedAt)).toEqual({ kind: 'close', number: 634 });
		expect(decideIssueAction(clean, undefined, checkedAt)).toEqual({ kind: 'none' });
	});

	describe('through gh', () => {
		function fakeGh(listing: string) {
			const calls: { args: string[]; input?: string }[] = [];
			const gh: Gh = (args, input) => {
				calls.push({ args, input });
				if (args.includes('POST')) return '{"number":700}';
				return args.includes('--paginate') ? listing : '';
			};
			return { gh, calls };
		}
		const listing = (title: string, body: string) =>
			`${JSON.stringify({ number: 5, title: 'other', body: '' })}\n${JSON.stringify({ number: 634, title, body })}\n`;

		it('creates through REST when no open issue has the title', () => {
			const { gh, calls } = fakeGh(listing('another title', ''));
			expect(syncIssue(report, gh, 'o/r', checkedAt)).toBe('created issue #700');
			expect(calls[1].args).toEqual([
				'api',
				'--method',
				'POST',
				'repos/o/r/issues',
				'--input',
				'-',
			]);
			expect(JSON.parse(calls[1].input ?? '')).toEqual({
				title: ISSUE_TITLE,
				body: issueBody(report, checkedAt),
			});
		});

		it('patches the body of the issue with the title when the commits differ', () => {
			const { gh, calls } = fakeGh(listing(ISSUE_TITLE, 'Checked x UTC against pin 1.'));
			expect(syncIssue(report, gh, 'o/r', checkedAt)).toBe('updated issue #634');
			expect(calls[1].args.slice(0, 4)).toEqual([
				'api',
				'--method',
				'PATCH',
				'repos/o/r/issues/634',
			]);
		});

		it('writes nothing when the issue is current', () => {
			const { gh, calls } = fakeGh(listing(ISSUE_TITLE, issueBody(report, checkedAt)));
			expect(syncIssue(report, gh, 'o/r', checkedAt)).toBe(
				'issue #634 already lists these commits',
			);
			expect(calls).toHaveLength(1);
		});

		it('closes as completed when no commits remain', () => {
			const { gh, calls } = fakeGh(listing(ISSUE_TITLE, 'x'));
			syncIssue({ ...report, commits: [] }, gh, 'o/r', checkedAt);
			expect(JSON.parse(calls[1].input ?? '')).toEqual({
				state: 'closed',
				state_reason: 'completed',
			});
		});
	});
});
