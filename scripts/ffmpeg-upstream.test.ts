import { cpSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';

import {
	compareUsedFiles,
	dependencyPaths,
	findContainingRelease,
	type Gh,
	movePin,
	parseCommits,
	parseReleaseTags,
	parseUsedFiles,
	releaseBranchForTag,
	releaseCheckComment,
	renderReport,
	renderUsedFiles,
	type Report,
	syncReleaseCheck,
	watchedPaths,
	withNoticeCommit,
} from './ffmpeg-upstream';

const sysCrate = path.resolve(import.meta.dirname, '../vendor/ffmpeg-sys-next-9.0.1');
const buildRs = readFileSync(path.join(sysCrate, 'build.rs'), 'utf8');
const provenance = readFileSync(path.join(sysCrate, 'ABB-PROVENANCE.md'), 'utf8');

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

function buildRsComponents(text: string): string[] {
	return [...text.matchAll(/\(\s*"([a-z]+)",\s*&\[([^\]]*)\]/g)].flatMap(([, kind, names]) =>
		[...names.matchAll(/"([a-z0-9_]+)"/g)].map(([, name]) => `${kind} ${name}`),
	);
}

describe('the component lists in build.rs', () => {
	it('match the provenance table, apart from what configure selects', () => {
		const selected = /const CONFIGURE_SELECTED[\s\S]*?\n\];/.exec(buildRs)?.[0] ?? '';
		const listed = new Set(
			buildRsComponents(buildRs).filter(
				(component) => !buildRsComponents(selected).includes(component),
			),
		);
		expect(listed.size).toBeGreaterThan(0);
		expect(new Set(tableComponents(provenance))).toEqual(listed);
	});
});

describe('the FFmpeg files a build compiled', () => {
	it('reads every source and header a dependency file names, leaving out system headers', () => {
		const dependencies = [
			'libavformat/aacdec.o: \\',
			'  /Applications/Xcode.app/SDKs/MacOSX.sdk/usr/include/stdio.h \\',
			'  libavformat/aacdec.c ./libavutil/avassert.h config.h \\',
			'  libavutil/aarch64/intreadwrite.h',
			'libavutil/avassert.h:',
		].join('\n');
		expect(dependencyPaths(dependencies)).toEqual([
			'libavformat/aacdec.c',
			'libavutil/avassert.h',
			'config.h',
			'libavutil/aarch64/intreadwrite.h',
		]);
	});

	it('names what a build compiled that the list lacks, and what the pinned source no longer has', () => {
		const tracked = new Set([
			'libavcodec/aacenc.c',
			'libavcodec/x86/aacencdsp.asm',
			'libavutil/log.h',
		]);
		expect(
			compareUsedFiles(
				['libavcodec/aacenc.c', 'libavformat/vorbiscomment.c'],
				new Set(['libavcodec/aacenc.c', 'libavcodec/x86/aacencdsp.asm']),
				tracked,
			),
		).toEqual({
			unlisted: ['libavcodec/x86/aacencdsp.asm'],
			gone: ['libavformat/vorbiscomment.c'],
		});
	});

	it('writes the list sorted, once per file', () => {
		expect(renderUsedFiles(['b.c', 'a.h', 'b.c'])).toBe('a.h\nb.c\n');
		expect(parseUsedFiles('a.h\nb.c\n')).toEqual(['a.h', 'b.c']);
	});
});

describe('the watched paths', () => {
	it('follow a listed file to the name a rename gave it, and ignore renames of unlisted files', () => {
		const renames = [
			'R100\tlibavformat/vorbiscomment.c\tlibavformat/vorbis/comment/write.c',
			'R097\tlibavcodec/x86/cabac.h\tlibavcodec/x86/cabac_new.h',
		].join('\n');
		expect(watchedPaths(['libavformat/vorbiscomment.c', 'libavformat/movenc.c'], renames)).toEqual([
			':(literal)libavformat/vorbiscomment.c',
			':(literal)libavformat/movenc.c',
			':(literal)libavformat/vorbis/comment/write.c',
		]);
		expect(watchedPaths(['libavformat/movenc.c'], '')).toEqual([':(literal)libavformat/movenc.c']);
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
	});
});

describe('the weekly release-tag check', () => {
	const pin = '05c674c9132eb13e65e3fb6e9b4cf1cad1219fd5';
	const refs = (...tags: string[]) => tags.map((tag) => `refs/tags/${tag}`).join('\n');

	it('sorts formal release tags at or above major 9 newest-first and ignores dev tags', () => {
		expect(
			parseReleaseTags(
				refs(
					'n8.1.3',
					'n9.0',
					'n9.0.2',
					'n9.1',
					'n9.1-dev',
					'n9.2-dev',
					'n9.2',
					'n9.2.10',
					'n10.0',
				),
			),
		).toEqual(['n10.0', 'n9.2.10', 'n9.2', 'n9.1', 'n9.0.2', 'n9.0']);
	});

	it('maps release tags to their upstream release branch', () => {
		expect(releaseBranchForTag('n9.2')).toBe('release/9.2');
		expect(releaseBranchForTag('n9.2.1')).toBe('release/9.2');
		expect(() => releaseBranchForTag('n9.2-dev')).toThrow("Not an FFmpeg release tag: 'n9.2-dev'");
	});

	function fakeReleaseGh(options: {
		issueState?: string;
		tags: string[];
		compareStatus: Record<string, string>;
		comments?: string[];
	}) {
		const calls: { args: string[]; input?: string }[] = [];
		const gh: Gh = (args, input) => {
			calls.push({ args, input });
			if (args.includes('POST')) return '{}';
			const endpoint = args.find((arg) => arg.startsWith('repos/')) ?? '';
			if (endpoint === 'repos/o/r/issues/669') return options.issueState ?? 'open';
			if (endpoint === 'repos/FFmpeg/FFmpeg/git/matching-refs/tags/n') {
				return refs(...options.tags);
			}
			const compareMatch = /repos\/FFmpeg\/FFmpeg\/compare\/[^.]+\.\.\.([^?]+)/.exec(endpoint);
			if (compareMatch) {
				return options.compareStatus[compareMatch[1]] ?? 'diverged';
			}
			if (endpoint.startsWith('repos/o/r/issues/669/comments')) {
				return (options.comments ?? []).map((body) => JSON.stringify({ body })).join('\n');
			}
			throw new Error(`Unexpected gh call: ${args.join(' ')}`);
		};
		return { gh, calls };
	}

	it('returns null when every release tag is diverged or behind the pin', () => {
		const { gh } = fakeReleaseGh({
			tags: ['n9.0', 'n9.1', 'n9.2-dev'],
			compareStatus: { 'n9.0': 'behind', 'n9.1': 'diverged' },
		});
		expect(findContainingRelease(pin, gh)).toBeNull();
	});

	it('returns the newest release tag whose compare status is ahead or identical', () => {
		const { gh } = fakeReleaseGh({
			tags: ['n9.1', 'n9.2', 'n9.2.1'],
			compareStatus: { 'n9.1': 'diverged', 'n9.2': 'identical', 'n9.2.1': 'ahead' },
		});
		expect(findContainingRelease(pin, gh)).toBe('n9.2.1');
	});

	it('stays silent when no release tag contains the pin', () => {
		const { gh, calls } = fakeReleaseGh({
			tags: ['n9.1', 'n9.2-dev'],
			compareStatus: { 'n9.1': 'diverged' },
		});
		expect(syncReleaseCheck(pin, 669, gh, 'o/r')).toBe(
			`no FFmpeg release tag contains pin ${pin.slice(0, 10)}`,
		);
		expect(calls.some((call) => call.args.includes('POST'))).toBe(false);
	});

	it('posts one comment on the tracking issue once a release tag contains the pin', () => {
		const { gh, calls } = fakeReleaseGh({
			tags: ['n9.1', 'n9.2'],
			compareStatus: { 'n9.1': 'diverged', 'n9.2': 'ahead' },
		});
		expect(syncReleaseCheck(pin, 669, gh, 'o/r')).toBe(
			'commented on issue #669 for n9.2 (release/9.2)',
		);
		const post = calls.find((call) => call.args.includes('POST'));
		expect(post?.args).toEqual([
			'api',
			'--method',
			'POST',
			'repos/o/r/issues/669/comments',
			'--input',
			'-',
		]);
		expect(JSON.parse(post?.input ?? '')).toEqual({
			body: releaseCheckComment('n9.2', pin),
		});
	});

	it('skips commenting when the tracking issue already notes that release branch', () => {
		const { gh, calls } = fakeReleaseGh({
			tags: ['n9.2', 'n9.2.1'],
			compareStatus: { 'n9.2': 'ahead', 'n9.2.1': 'ahead' },
			comments: [releaseCheckComment('n9.2', pin)],
		});
		expect(syncReleaseCheck(pin, 669, gh, 'o/r')).toBe('issue #669 already notes release/9.2');
		expect(calls.some((call) => call.args.includes('POST'))).toBe(false);
	});

	it('skips the check when the tracking issue is already closed', () => {
		const { gh, calls } = fakeReleaseGh({
			issueState: 'closed',
			tags: ['n9.2'],
			compareStatus: { 'n9.2': 'ahead' },
		});
		expect(syncReleaseCheck(pin, 669, gh, 'o/r')).toBe(
			'issue #669 is closed; skipping release check',
		);
		expect(calls).toHaveLength(1);
	});
});

describe('moving the pin', () => {
	const repoRoot = path.resolve(import.meta.dirname, '..');
	const revision = '49a3b1f259aa00bb11cc22dd33ee44ff55667788';
	const scratch: string[] = [];
	afterEach(() => {
		for (const dir of scratch.splice(0)) rmSync(dir, { recursive: true, force: true });
	});

	function fakeRepo() {
		const root = mkdtempSync(path.join(os.tmpdir(), 'abb-ffmpeg-pin-'));
		scratch.push(root);
		for (const file of [
			'scripts/setup.sh',
			'vendor/ffmpeg-sys-next-9.0.1/ffmpeg-revision',
			'vendor/ffmpeg-sys-next-9.0.1/ABB-PROVENANCE.md',
		]) {
			cpSync(path.join(repoRoot, file), path.join(root, file));
		}
		return root;
	}
	const read = (root: string, file: string) => readFileSync(path.join(root, file), 'utf8');
	const gh: Gh = () => revision;

	it('updates ffmpeg-revision and ABB-PROVENANCE.md without touching setup.sh', () => {
		const root = fakeRepo();
		const setupBefore = read(root, 'scripts/setup.sh');
		expect(movePin(root, '49a3b1f259', gh)).toBe(`Pinned FFmpeg ${revision}`);
		expect(read(root, 'vendor/ffmpeg-sys-next-9.0.1/ffmpeg-revision')).toBe(`${revision}\n`);
		expect(read(root, 'vendor/ffmpeg-sys-next-9.0.1/ABB-PROVENANCE.md')).toContain(
			`built from FFmpeg commit \`${revision}\``,
		);
		expect(read(root, 'scripts/setup.sh')).toBe(setupBefore);
	});

	it('accepts an FFmpeg release tag such as n9.2', () => {
		const root = fakeRepo();
		const seen: string[][] = [];
		const tagGh: Gh = (args) => {
			seen.push(args);
			return revision;
		};
		expect(movePin(root, 'n9.2', tagGh)).toBe(`Pinned FFmpeg ${revision}`);
		expect(seen[0]).toEqual(['api', 'repos/FFmpeg/FFmpeg/commits/n9.2', '--jq', '.sha']);
	});

	it('rejects a non-commit, non-release ref without changing files', () => {
		const root = fakeRepo();
		expect(() => movePin(root, 'master', gh)).toThrow(
			"ffmpeg:pin needs an FFmpeg commit hash or release tag (nX.Y or nX.Y.Z), got 'master'.",
		);
		expect(read(root, 'vendor/ffmpeg-sys-next-9.0.1/ffmpeg-revision')).toBe(
			read(repoRoot, 'vendor/ffmpeg-sys-next-9.0.1/ffmpeg-revision'),
		);
	});

	it('names the pinned commit in the notice the app bundle ships, since the bundle lacks ffmpeg-revision', () => {
		const pinned = read(repoRoot, 'vendor/ffmpeg-sys-next-9.0.1/ffmpeg-revision').trim();
		expect(provenance).toContain(`built from FFmpeg commit \`${pinned}\``);
	});

	it('refuses to move a notice that does not name the pinned commit', () => {
		expect(() => withNoticeCommit('built from FFmpeg master', 'a'.repeat(40), revision)).toThrow(
			`does not name the pinned commit ${'a'.repeat(40)}`,
		);
	});
});
