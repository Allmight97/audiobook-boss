import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import {
	cargoPinFindings,
	changedSampleFindings,
	checkFile,
	frozenSampleFindings,
	packagePinFindings,
} from './check-known-mistakes';

const DISABLE = ['oxlint', 'disable'].join('-');

function found(file: string, text: string): string[] {
	return checkFile(file, text).map((finding) => `${finding.line}: ${finding.message}`);
}

describe('check-known-mistakes', () => {
	it('rejects a cover image outside CoverImage', () => {
		expect(found('src/ui/coverArt/CoverArtView.tsx', '\t<img src={url()} />')).toEqual([
			expect.stringMatching(/^1: .*outside CoverImage/),
		]);
		expect(found('src/ui/foundation/CoverImage.tsx', '\t<img src={url()} />')).toEqual([]);
	});

	it('rejects host OS wording outside the platform helper', () => {
		expect(found('src/ui/QuitReminder.tsx', "\tconst key = '⌘Q';")).toEqual([
			expect.stringMatching(/^1: .*src\/lib\/platform/),
		]);
		expect(found('src/app/x.ts', "\tnavigator.userAgent.includes('Mac');")).toHaveLength(1);
		expect(found('src/lib/platform/index.ts', "export const QUIT_SHORTCUT = '⌘Q';")).toEqual([]);
		expect(found('src/ui/x.test.tsx', "\tvi.spyOn(navigator, 'userAgent', 'get');")).toEqual([]);
	});

	it('rejects a lint suppression without a reason', () => {
		expect(found('src/x.ts', `// ${DISABLE}-next-line no-explicit-any`)).toHaveLength(1);
		expect(
			found('scripts/x.ts', `/* ${DISABLE} typescript/no-unsafe-type-assertion */`),
		).toHaveLength(1);
		expect(found('src/x.ts', `// ${DISABLE}-next-line no-explicit-any -- generated shape`)).toEqual(
			[],
		);
		expect(found('src/x.ts', '/* oxlint-enable typescript/no-unsafe-type-assertion */')).toEqual(
			[],
		);
	});

	it('rejects a uvx call without the release wait', () => {
		expect(found('scripts/x.sh', 'tool=(uvx lizard)')).toHaveLength(1);
		expect(found('scripts/x.sh', 'out="$(uvx zizmor .)"')).toHaveLength(1);
		expect(found('scripts/x.sh', 'tool=(uvx --exclude-newer "10 days" lizard)')).toEqual([]);
		expect(found('scripts/x.sh', 'if ! command -v uvx >/dev/null 2>&1; then')).toEqual([]);
		expect(found('scripts/x.sh', 'for tool in actionlint uvx bun; do')).toEqual([]);
	});

	it('rejects a write to a shell profile', () => {
		expect(found('scripts/setup.sh', 'echo "export PATH" >> "$HOME/.zshrc"')).toHaveLength(1);
		expect(found('scripts/setup.sh', 'printf x | tee -a ~/.bashrc')).toHaveLength(1);
		expect(found('scripts/setup.sh', 'printf \'%s\\n\' "$dir" >>"$GITHUB_PATH"')).toEqual([]);
	});

	it('sends every output header through its owner', () => {
		expect(
			found('crates/abb-engine/src/metadata/remux.rs', '    octx.write_header_with(options)'),
		).toHaveLength(1);
		expect(
			found('crates/abb-engine/src/audio/processor/merge.rs', 'output.write_header()?;'),
		).toHaveLength(1);
		expect(
			found('crates/abb-engine/src/audio/output_header.rs', 'octx.write_header_with(options)?;'),
		).toEqual([]);
		expect(
			found('crates/abb-engine/src/metadata/remux.rs', 'write_output_header(octx, options)'),
		).toEqual([]);
	});

	it('keeps engine owners inside their boundaries', () => {
		expect(
			found('crates/abb-engine/src/metadata/save.rs', 'use crate::audio::AudioFile;'),
		).toHaveLength(1);
		expect(
			found(
				'crates/abb-engine/src/metadata/save.rs',
				'use crate::audio::{AudioIntent, AudioFile};',
			),
		).toHaveLength(1);
		expect(
			found('crates/abb-engine/src/metadata/save.rs', 'use crate::audio::AudioIntent;'),
		).toEqual([]);
		expect(
			found(
				'crates/abb-engine/src/audio/processor/finalize.rs',
				'use crate::processing::ProcessingContext;',
			),
		).toHaveLength(1);
		expect(
			found(
				'crates/abb-engine/src/audio/processor/finalize.rs',
				'use crate::execution::ProcessingContext;',
			),
		).toEqual([]);
		expect(
			found('crates/abb-engine/src/processing/run.rs', 'use crate::remote_source::RemoteTitle;'),
		).toHaveLength(1);
		expect(
			found('crates/abb-engine/src/session/state.rs', 'use crate::remote_source::RemoteTitle;'),
		).toEqual([]);
		expect(
			found(
				'crates/abb-engine/src/work_runtime/runtime.rs',
				'use crate::remote_source::materializer::Plan;',
			),
		).toHaveLength(1);
		expect(
			found('crates/abb-engine/src/work_runtime/runtime.rs', 'use crate::processing::run;'),
		).toEqual([]);
	});

	it('keeps final artifact moves inside output_artifact', () => {
		const processor = 'crates/abb-engine/src/audio/processor/finalize.rs';
		expect(found(processor, '\tstd::fs::copy(&staged, &final_path)?;')).toEqual([
			expect.stringMatching(/^1: `fs::copy` moves a file outside the output artifact boundary/),
		]);
		expect(found(processor, '\ttokio::fs::hard_link(a, b).await?;')).toHaveLength(1);
		expect(found(processor, '\tlet name = "fs::rename";\n\t// fs::copy')).toEqual([]);
		expect(
			found('crates/abb-engine/src/audio/processor/finalize_tests.rs', 'fs::copy(a, b)?;'),
		).toEqual([]);
		expect(found('crates/abb-engine/src/output_artifact/commit.rs', 'fs::copy(a, b)?;')).toEqual(
			[],
		);
	});

	it('keeps file I/O out of the session state modules', () => {
		expect(
			found('crates/abb-engine/src/session/state.rs', 'let bytes = std::fs::read(path)?;'),
		).toEqual([expect.stringMatching(/^1: `std::fs` does file I\/O in a session state module/)]);
		expect(
			found('crates/abb-engine/src/session/plans.rs', 'let file = File::open(path)?;'),
		).toHaveLength(1);
		expect(
			found('crates/abb-engine/src/session/runtime.rs', 'let file = File::open(path)?;'),
		).toEqual([]);
		expect(
			found('crates/abb-engine/src/session/runtime/save_flow.rs', 'let file = File::open(path)?;'),
		).toEqual([]);
		expect(found('crates/abb-engine/src/session/state_tests.rs', 'std::fs::read(path)?;')).toEqual(
			[],
		);
	});

	it('keeps product rules out of the fake engine', () => {
		const fake = 'src/test/fixtures/fakeEngine.ts';
		expect(
			found(fake, "\t\tif (!answered && intent.kind === 'import') appendFiles(paths);"),
		).toEqual([]);
		expect(found(fake, "\t\tif (intent.kind === 'setField') rewrite(intent);")).toEqual([
			expect.stringMatching(/^1: `intent\.kind` puts a product rule in the fake engine/),
		]);
		expect(found(fake, '\t\tswitch (intent.kind) {')).toHaveLength(1);
		expect(found(fake, '\t\tselection.canGroup = selection.selectedIndices.length >= 2;')).toEqual([
			expect.stringMatching(/^1: `\.canGroup =` puts a product rule in the fake engine/),
		]);
		expect(found(fake, '\t\ttitles.titleTotals = {};')).toHaveLength(1);
		expect(found(fake, '\t\tif (selection.canGroup === true) pass();')).toEqual([]);
		expect(
			found('src/test/fixtures/recordedInvoke.ts', "if (intent.kind === 'setField') x();"),
		).toEqual([]);
	});

	it('rejects comments that narrate history', () => {
		expect(found('src/app/x.ts', '// Advances as it did before the rewrite.')).toEqual([
			expect.stringMatching(/^1: `as it did` is a comment that narrates history/),
		]);
		expect(found('src-tauri/Cargo.toml', '# formerly pinned by hand')).toHaveLength(1);
		expect(
			found(
				'crates/abb-engine/src/x.rs',
				'/// Replaces the old store, dropped because it was heavy.',
			),
		).toHaveLength(1);
		expect(found('scripts/x.sh', '# d7c41f03:scripts/setup.sh:24 is the cause')).toHaveLength(1);
		expect(found('src/app/x.ts', '// A reply that lands after disposal changes nothing.')).toEqual(
			[],
		);
		expect(found('src/app/x.ts', 'const note = "as it did";')).toEqual([]);
		expect(
			found('crates/abb-engine/src/x.rs', '/// A read begun before the save can no longer land.'),
		).toEqual([]);
	});

	it('rejects Rust comments that excuse code, mislabel safety, or claim what nothing enforces', () => {
		const file = 'crates/abb-engine/src/x.rs';
		expect(found(file, '    // shortcut: upgrade this later')).toEqual([
			expect.stringMatching(/^1: `shortcut` is a comment that excuses code/),
		]);
		expect(found(file, '    // TODO tidy this')).toHaveLength(1);
		expect(found(file, '//! These behaviors are intentional architecture.')).toHaveLength(1);
		expect(found(file, '    // Safety: ensure the path passes validation.')).toHaveLength(1);
		expect(found(file, '    // SAFETY: the pointer is valid for the call.')).toEqual([]);
		expect(found(file, '    /// Cleans up and never panics.')).toHaveLength(1);
		expect(found(file, '    /// Seconds (None if unavailable)')).toHaveLength(1);
		expect(found(file, '    /// Seconds.')).toEqual([]);
		expect(found(file, '/// Names the rule (#299 Branch A).')).toHaveLength(1);
		expect(found(file, '/// Names the rule (#299).')).toEqual([]);
		expect(found(file, '#[expect(lint, reason = "joined by the test that starts it")]')).toEqual([
			expect.stringMatching(/^1: `joined by the ` gives a vague join reason/),
		]);
		expect(found(file, '#[expect(lint, reason = "joined by run_job")]')).toEqual([]);
	});

	it('requires a pin tag above each exact Cargo pin', () => {
		const tag = '# pin: prerelease family; unpin when specta 2.0.0 is stable';
		expect(cargoPinFindings('crates/x/Cargo.toml', `${tag}\nspecta = "=2.0.0-rc.25"`)).toEqual([]);
		expect(
			cargoPinFindings(
				'crates/x/Cargo.toml',
				`${tag}\nspecta = "=2.0.0-rc.25"\nspecta-typescript = "=0.0.12"`,
			),
		).toEqual([]);
		expect(cargoPinFindings('crates/x/Cargo.toml', 'specta = "=2.0.0-rc.25"')).toEqual([
			expect.objectContaining({
				line: 1,
				message: expect.stringContaining('exact pin without a tag'),
			}),
		]);
		expect(
			cargoPinFindings('crates/x/Cargo.toml', `${tag}\n\nspecta = "=2.0.0-rc.25"`),
		).toHaveLength(1);
		expect(
			cargoPinFindings('crates/x/Cargo.toml', '# pin: no trigger\nspecta = "=1.0.0"'),
		).toHaveLength(1);
		expect(cargoPinFindings('crates/x/Cargo.toml', 'serde = "1.0"')).toEqual([]);
		expect(cargoPinFindings('vendor/x/Cargo.toml', 'specta = "=2.0.0-rc.25"')).toEqual([]);
	});

	it('requires a pin-table row for each exact package pin and override', () => {
		const manifest = JSON.stringify({
			dependencies: { 'solid-js': '2.0.0-rc.18' },
			devDependencies: { typescript: 'npm:typescript@7.0.2', vitest: '^4.0.0' },
			overrides: { undici: '^7.30.1' },
		});
		const rows = [
			'| `package.json` | `solid-js` | pin: a; unpin when b |',
			'| `package.json` | `typescript` | pin: a; unpin when b |',
			'| `package.json` | `undici (override)` | pin: a; unpin when b |',
		];
		expect(packagePinFindings(manifest, rows.join('\n'))).toEqual([]);
		expect(packagePinFindings(manifest, rows.slice(1).join('\n'))).toEqual([
			expect.objectContaining({ message: expect.stringContaining('`solid-js` is an exact pin') }),
		]);
		expect(packagePinFindings(manifest, rows.slice(0, 2).join('\n'))).toEqual([
			expect.objectContaining({ message: expect.stringContaining('`undici (override)`') }),
		]);
	});

	it('keeps pure cores free of I/O and provider names', () => {
		expect(
			found('crates/abb-media-core/src/lib.rs', 'let bytes = std::fs::read(path)?;'),
		).toHaveLength(1);
		expect(found('crates/abb-media-core/src/lib.rs', 'tokio::spawn(work);')).toHaveLength(1);
		expect(
			found('crates/abb-media-core/src/lib_tests.rs', 'let bytes = std::fs::read(path)?;'),
		).toEqual([]);
		expect(found('crates/abb-engine/src/audio/x.rs', 'let bytes = std::fs::read(path)?;')).toEqual(
			[],
		);
		expect(
			found('crates/abb-remote-source-core/src/lib.rs', 'pub fn audible_region() {}'),
		).toHaveLength(1);
		expect(
			found('crates/abb-remote-source-core/src/lib.rs', 'pub fn provider_region() {}'),
		).toEqual([]);
	});

	it('pairs each `mod tests;` with its sibling test file', () => {
		const declare = (pathLine: string) => `#[cfg(test)]\n${pathLine}\nmod tests;`;
		expect(
			found('crates/abb-engine/src/cover_source.rs', declare('#[path = "cover_source_tests.rs"]')),
		).toEqual([]);
		expect(
			found('crates/abb-engine/src/cover_source.rs', declare('#[path = "other_tests.rs"]')),
		).toEqual([
			expect.stringMatching(/^3: .*loads other_tests.rs, not the sibling cover_source_tests.rs/),
		]);
		expect(found('crates/abb-engine/src/cover_source.rs', '#[cfg(test)]\nmod tests;')).toHaveLength(
			1,
		);
		expect(
			found('crates/abb-engine/src/metadata/mod.rs', declare('#[path = "metadata_tests.rs"]')),
		).toEqual([]);
		expect(
			found('crates/abb-engine/src/processing/job_registry/mod.rs', '#[cfg(test)]\nmod tests;'),
		).toEqual([]);
		expect(
			found('crates/abb-engine/src/bin/abb_dev.rs', declare('#[path = "abb_dev/tests.rs"]')),
		).toEqual([]);
	});

	it('rejects destructured Solid component props', () => {
		const file = 'src/ui/probe/Probe.tsx';
		expect(found(file, 'export function Probe({ name }: Props): JSX.Element {')).toEqual([
			expect.stringMatching(/^1: .*destructures Solid component props/),
		]);
		expect(found(file, 'const Probe = ({ name }: Props) => <p>{name}</p>;')).toHaveLength(1);
		expect(found(file, '\tconst { name, size } = props;')).toHaveLength(1);
		expect(found(file, 'export function Probe(props: Props): JSX.Element {')).toEqual([]);
		expect(found(file, '\tconst { name } = options;')).toEqual([]);
		expect(found('src/lib/probe.ts', 'export function Probe({ name }: Props) {')).toEqual([]);
	});

	it('rejects GNU-only regex escapes in sed', () => {
		expect(found('scripts/setup.sh', `\tsed -n '2,12p' "$0" | sed 's/^# \\?//'`)).toEqual([
			expect.stringMatching(/^1: .*GNU-only/),
		]);
		expect(
			found(
				'scripts/setup.sh',
				`\tsed -n 's/.*version n\\?\\([0-9][0-9]*\\).*/\\1/p' <<<"\${line}"`,
			),
		).toHaveLength(1);
		expect(found('scripts/setup.sh', `\tsed -E 's/^# ?//'`)).toEqual([]);
	});

	it('rejects other GNU or bash 4 forms unless marked', () => {
		expect(
			found('scripts/x.sh', 'sed -i "s/a/b/" file\nreadlink -f x\ndeclare -A map'),
		).toHaveLength(3);
		expect(
			found('scripts/x.sh', '# allow-gnu: runs only in the Linux container\nsed -i "s/a/b/" file'),
		).toEqual([]);
	});

	it('rejects || true inside a multi-line command substitution', () => {
		const old = [
			'\thits="$(',
			'\t\tcargo tree --locked -p "$crate" --format \'{p}\' |',
			'\t\t\tawk \'{ print $1 }\' | sort -u | grep -E "$forbidden" || true',
			'\t)"',
		].join('\n');
		expect(found('scripts/check-rust-tiers.sh', old)).toEqual([
			expect.stringMatching(/^3: `\|\| true` hides an error/),
		]);
	});

	it('rejects a silenced single-line capture unless marked with a reason', () => {
		expect(found('scripts/x.sh', 'v="$(tool --version 2>/dev/null)"')).toHaveLength(1);
		expect(
			found('scripts/x.sh', 'v="$(tool --version 2>/dev/null)" # allow-silence: optional probe'),
		).toEqual([]);
		expect(found('scripts/x.sh', 'kill "$pid" 2>/dev/null || true')).toEqual([]);
	});

	it('rejects a permission-bit failure fake in Rust, inline test modules included', () => {
		const line =
			'        std::fs::set_permissions(temp.path(), std::fs::Permissions::from_mode(0o500))';
		expect(
			found('crates/abb-engine/src/remote_source/providers/indexer/connection.rs', line),
		).toEqual([expect.stringMatching(/^1: `from_mode\(0o500\)` fakes an I\/O failure/)]);
		expect(found('crates/abb-engine/src/x.rs', 'permissions.set_mode(0o755);')).toEqual([]);
		expect(
			found(
				'crates/abb-engine/tests/all_tests.rs',
				`// allow-permission-fake: runs as nobody\n${line}`,
			),
		).toEqual([]);
	});

	it('rejects comments that stand in for structure or enforcement', () => {
		expect(found('crates/abb-engine/src/session/state.rs', '// ---- Titles ----')).toEqual([
			expect.stringMatching(/^1: `---- Titles ----` is a comment that stands in/),
		]);
		expect(found('src-tauri/src/lib.rs', '// EXCEPTION: tiny helper inline test.')).toHaveLength(1);
		expect(found('src-tauri/Cargo.toml', '# DO NOT REMOVE!!')).toHaveLength(1);
		expect(found('src/app/x.ts', '/* ==== Layout ==== */')).toHaveLength(1);
		expect(found('src/app/x.ts', "const rule = '---';\n// Titles")).toEqual([]);
		expect(found('docs/x.md', '<!-- ---- Section ---- -->')).toEqual([]);
	});

	it('rejects a Rust test outside a test file', () => {
		const inline = '#[cfg(test)]\nmod tests {\n    #[test]\n    fn it_works() {}\n}';
		expect(found('crates/abb-engine/src/session/state.rs', inline)).toEqual([
			expect.stringMatching(/^3: `#\[test\]` is a rust test outside a test file/),
		]);
		expect(found('src-tauri/src/lib.rs', '    #[tokio::test]\n    async fn t() {}')).toHaveLength(
			1,
		);
		expect(found('crates/abb-engine/src/x.rs', 'proptest! {\n    fn p() {}\n}')).toHaveLength(1);
		expect(found('crates/abb-engine/src/session/state_tests.rs', inline)).toEqual([]);
		expect(found('crates/abb-engine/src/processing/job_registry/tests.rs', inline)).toEqual([]);
		expect(found('crates/abb-engine/src/test_cases/unit_x.rs', inline)).toEqual([]);
		expect(found('crates/abb-engine/tests/all_tests.rs', inline)).toEqual([]);
		expect(found('crates/abb-engine/src/x.rs', '// #[test]\nlet s = "#[test]";')).toEqual([]);
	});

	it('rejects unsafe in the engine outside the FAAC and FAAD3 adapters', () => {
		const block = 'let v = unsafe { read(p) };';
		expect(found('crates/abb-engine/src/session/state.rs', block)).toEqual([
			expect.stringMatching(/^1: `unsafe \{` is unsafe in the engine/),
		]);
		expect(found('crates/abb-engine/src/metadata/remux.rs', block)).toHaveLength(1);
		expect(found('crates/abb-engine/src/audio/buffer.rs', block)).toHaveLength(1);
		expect(
			found('crates/abb-engine/src/x.rs', 'pub unsafe fn f() {}\nunsafe impl Send for X {}'),
		).toHaveLength(2);
		expect(found('crates/abb-engine/src/audio/processor/encoder/faac.rs', block)).toEqual([]);
		expect(found('crates/abb-engine/src/audio/processor/faad_decoder.rs', block)).toEqual([]);
		expect(found('crates/abb-engine/src/audio/processor/encoder/faac_tests.rs', block)).toEqual([]);
		expect(found('crates/abb-media-core/src/lib.rs', block)).toEqual([]);
		expect(
			found(
				'crates/abb-engine/src/x.rs',
				'// unsafe { }\nlet s = "unsafe { not code }";\nlet unsafe_title = 1;',
			),
		).toEqual([]);
	});

	it('keeps raw pointers out of the public abb-ffmpeg surface', () => {
		const file = 'crates/abb-ffmpeg/src/output.rs';
		expect(found(file, 'pub fn as_ptr(&self) -> *mut sys::AVFormatContext {')).toEqual([
			expect.stringMatching(/^1: .*raw pointer in the public abb-ffmpeg surface/),
		]);
		expect(
			found(
				file,
				'pub fn open(\n    path: &Path,\n    ctx: *const sys::AVFormatContext,\n) -> Self {',
			),
		).toHaveLength(1);
		expect(found(file, 'pub struct S {\n    pub ctx: *mut sys::AVFrame,\n}')).toHaveLength(1);
		expect(found(file, 'pub struct S {\n    pub ctx:\n        *mut sys::AVFrame,\n}')).toHaveLength(
			1,
		);
		expect(found(file, 'pub async fn run(ctx: *mut sys::AVFrame) -> Result<()> {')).toHaveLength(1);
		expect(found(file, 'pub type Raw = *mut sys::AVPacket;')).toHaveLength(1);
		expect(found(file, 'pub(crate) fn as_ptr(&self) -> *mut sys::AVFormatContext {')).toEqual([]);
		expect(
			found(file, 'pub fn create(path: &Path) -> Result<Self> {\n    let p: *mut u8 = ptr();'),
		).toEqual([]);
		expect(found('crates/abb-engine/src/x.rs', 'pub fn as_ptr(&self) -> *mut u8 {')).toEqual([]);
	});

	it('keeps output artifact types out of the processing re-exports', () => {
		expect(
			found('crates/abb-engine/src/processing.rs', 'pub use crate::output_artifact::OutputPlan;'),
		).toEqual([expect.stringMatching(/^1: .*re-exports output artifact types/)]);
		expect(
			found(
				'crates/abb-engine/src/processing/run.rs',
				'pub(crate) use crate::{audio::X, output_artifact::Y};',
			),
		).toHaveLength(1);
		expect(
			found(
				'crates/abb-engine/src/processing/run.rs',
				'pub(crate) use crate::{\n    audio::X,\n    output_artifact::{Y, Z},\n};',
			),
		).toEqual([expect.stringMatching(/^1: .*re-exports output artifact types/)]);
		expect(
			found(
				'crates/abb-engine/src/processing.rs',
				'// pub use crate::output_artifact::OutputPlan;',
			),
		).toEqual([]);
		expect(
			found('crates/abb-engine/src/processing.rs', 'use crate::output_artifact::OutputPlan;'),
		).toEqual([]);
		expect(
			found('crates/abb-engine/src/session.rs', 'pub use crate::output_artifact::OutputPlan;'),
		).toEqual([]);
	});

	it('keeps the frontend on the engine status, not the source flags', () => {
		expect(found('src/app/x.ts', '\tif (file.isValid) {')).toEqual([
			expect.stringMatching(/^1: .*Fix: branch on the title or source `status`/),
		]);
		expect(found('src/ui/x.tsx', '\t<Show when={source?.sourceRemoved}>')).toHaveLength(1);
		expect(found('src/app/x.ts', '\tconst { isValid, label } = file;')).toHaveLength(1);
		expect(found('src/app/x.ts', '\tfiles.filter(({ isValid }) => isValid);')).toHaveLength(1);
		expect(found('src/app/x.ts', "\tif (file['isValid']) {")).toHaveLength(1);
		expect(found('src/app/x.ts', "\t\tcase 'sourceRemoved':")).toEqual([]);
		expect(found('src/app/x.ts', "\tif (file.status === 'ready') {")).toEqual([]);
		expect(found('src/test/fixtures/fakeEngine.ts', '\t\tisValid: true,')).toEqual([]);
		expect(found('src/app/x.test.ts', '\texpect(file.isValid).toBe(true);')).toEqual([]);
	});

	it('keeps FFmpeg format name comparisons in the two classifiers', () => {
		const file = 'crates/abb-engine/src/audio/file_list.rs';
		expect(
			found(file, '    if !input.format_name().split(\',\').any(|name| name == "mp4") {'),
		).toEqual([expect.stringMatching(/^1: .*compares an FFmpeg format or muxer name/)]);
		expect(
			found(file, '            && container_name.split(\',\').any(|name| name == "matroska")'),
		).toHaveLength(1);
		expect(found(file, '    match container.muxer_name() {')).toHaveLength(1);
		expect(found(file, '    if input.format_name() == "mp4" {')).toHaveLength(1);
		expect(found(file, '        "adts" if !edited.is_empty() => refuse(),')).toHaveLength(1);
		expect(found(file, '    Output::create(path, output_format.muxer_name())')).toEqual([]);
		expect(found(file, '    let kept = matches!(extension, "m4a" | "m4b");')).toEqual([]);
		expect(found(file, '        .is_some_and(|e| e.eq_ignore_ascii_case("mp3"))')).toEqual([]);
		expect(found(file, '    // name == "mp4"')).toEqual([]);
		expect(
			found(
				'crates/abb-engine/src/metadata/container.rs',
				'            "matroska" => Container::Matroska,',
			),
		).toEqual([]);
		expect(
			found(
				'crates/abb-engine/src/audio/format.rs',
				'    name.split(\',\').any(|alias| alias == "mp4")',
			),
		).toEqual([]);
		expect(
			found('crates/abb-engine/src/audio/file_list_tests.rs', '    assert!(name == "matroska");'),
		).toEqual([]);
	});

	it('lets only src/styles.css import the foundation stylesheet', () => {
		expect(found('src/ui/x.tsx', "import './foundation/internal/foundation.css';")).toEqual([
			expect.stringMatching(/^1: .*imports the foundation stylesheet outside `src\/styles\.css`/),
		]);
		expect(found('src/ui/x.css', '@import "./foundation/internal/foundation.css";')).toHaveLength(
			1,
		);
		expect(found('src/styles.css', '@import "./ui/foundation/internal/foundation.css";')).toEqual(
			[],
		);
		expect(found('src/ui/x.tsx', "import './fileImport.css';")).toEqual([]);
	});

	it('keeps CSS classes out of frontend tests', () => {
		const file = 'src/ui/__tests__/x.test.tsx';
		expect(found(file, "\tconst row = container.querySelector('.file-row');")).toEqual([
			expect.stringMatching(/^1: `\.file-row` selects by CSS class/),
		]);
		expect(
			found(file, '\tconst rows = document.querySelectorAll<HTMLElement>(\n\t\t"li.row",\n\t);'),
		).toHaveLength(1);
		expect(found(file, "\texpect(row).toHaveClass('is-active');")).toHaveLength(1);
		expect(found(file, '\texpect(row.className).toContain("active");')).toHaveLength(1);
		expect(found(file, "\texpect(row.getAttribute('class')).toBe('x');")).toHaveLength(1);
		expect(found(file, "\texpect(row).toHaveAttribute('class', 'x');")).toHaveLength(1);
		expect(found(file, "\texpect(row).toHaveAttribute('data-state', 'x');")).toEqual([]);
		expect(found(file, '\tdocument.querySelectorAll(\'[data-testid="cover.png"]\');')).toEqual([]);
		expect(found(file, '\tdocument.querySelectorAll(\'[src$=".png"]\');')).toEqual([]);
		expect(found(file, "\tscreen.getByRole('button', { name: 'Save' });")).toEqual([]);
		expect(found('src/ui/x.tsx', "\tel.querySelector('.file-row');")).toEqual([]);
	});

	it('leaves vendored code alone', () => {
		expect(found('vendor/x/build.sh', "sed 's/a\\?//'")).toEqual([]);
	});
});

const SAMPLES = 'crates/abb-engine/src/app_settings/saved_settings';

function sampleRepo(body: (root: string, git: (...args: string[]) => string) => void): void {
	const root = mkdtempSync(path.join(os.tmpdir(), 'abb-samples-'));
	const git = (...args: string[]) => execFileSync('git', args, { cwd: root, encoding: 'utf8' });
	try {
		git('init', '-q', '-b', 'main');
		git('config', 'user.email', 'samples@example.com');
		git('config', 'user.name', 'Samples Test');
		git('config', 'commit.gpgsign', 'false');
		mkdirSync(path.join(root, SAMPLES), { recursive: true });
		writeFileSync(path.join(root, SAMPLES, 'first.toml'), '[keep_awake]\nenabled = true\n');
		git('add', '-A');
		git('commit', '-q', '-m', 'base', '--no-verify');
		body(root, git);
	} finally {
		rmSync(root, { recursive: true, force: true });
	}
}

describe('frozen settings samples', () => {
	it('allows a new sample and rejects a changed, deleted, or renamed one', () => {
		const dir = `${SAMPLES}/`;
		expect(frozenSampleFindings(`A\t${dir}new.toml`)).toEqual([]);
		expect(frozenSampleFindings(`M\tsrc/other.toml\nD\tcrates/x.toml`)).toEqual([]);
		for (const [entry, verb] of [
			[`M\t${dir}first.toml`, 'modified'],
			[`D\t${dir}first.toml`, 'deleted'],
			[`R100\t${dir}first.toml\t${dir}second.toml`, 'renamed'],
		]) {
			const [finding] = frozenSampleFindings(entry);
			expect(finding.file).toBe(`${dir}first.toml`);
			expect(finding.message).toContain(`was ${verb}`);
			expect(finding.message).toContain('frozen saved-settings files from real releases');
			expect(finding.message).toContain('add a new sample');
		}
	});

	it('fails on a staged edit, deletion, or rename and passes on a staged new sample', () => {
		sampleRepo((root, git) => {
			const first = path.join(root, SAMPLES, 'first.toml');
			writeFileSync(path.join(root, SAMPLES, 'added.toml'), '[keep_awake]\nenabled = false\n');
			git('add', '-A');
			expect(changedSampleFindings(root)).toEqual([]);
			git('reset', '-q');

			writeFileSync(first, '[keep_awake]\nenabled = false\n');
			git('add', '-A');
			expect(changedSampleFindings(root).map((finding) => finding.file)).toEqual([
				`${SAMPLES}/first.toml`,
			]);
			git('reset', '-q', '--hard');

			rmSync(first);
			git('add', '-A');
			expect(changedSampleFindings(root)[0].message).toContain('was deleted');
			git('reset', '-q', '--hard');

			renameSync(first, path.join(root, SAMPLES, 'renamed.toml'));
			git('add', '-A');
			expect(changedSampleFindings(root)[0].message).toContain('was renamed');
		});
	});

	it('allows editing a staged sample that an unpushed commit added', () => {
		sampleRepo((root, git) => {
			git('update-ref', 'refs/remotes/origin/main', 'HEAD');
			const added = path.join(root, SAMPLES, 'added.toml');
			writeFileSync(added, 'x = 1\n');
			git('add', '-A');
			git('commit', '-q', '-m', 'add', '--no-verify');
			writeFileSync(added, 'x = 2\n');
			git('add', '-A');
			expect(changedSampleFindings(root)).toEqual([]);
			writeFileSync(path.join(root, SAMPLES, 'first.toml'), 'x = 3\n');
			git('add', '-A');
			expect(changedSampleFindings(root).map((finding) => finding.file)).toEqual([
				`${SAMPLES}/first.toml`,
			]);
		});
	});

	it('rejects staging a deletion of a sample released before an unpushed commit', () => {
		sampleRepo((root, git) => {
			git('update-ref', 'refs/remotes/origin/main', 'HEAD');
			writeFileSync(path.join(root, 'other.txt'), 'x\n');
			git('add', '-A');
			git('commit', '-q', '-m', 'other', '--no-verify');
			rmSync(path.join(root, SAMPLES, 'first.toml'));
			git('add', '-A');
			expect(changedSampleFindings(root)[0].message).toContain('was deleted');
		});
	});

	it('compares a branch with the merge base when given a base', () => {
		sampleRepo((root, git) => {
			git('checkout', '-q', '-b', 'work');
			writeFileSync(path.join(root, SAMPLES, 'added.toml'), 'x = 1\n');
			git('add', '-A');
			git('commit', '-q', '-m', 'add', '--no-verify');
			expect(changedSampleFindings(root, 'main')).toEqual([]);
			writeFileSync(path.join(root, SAMPLES, 'first.toml'), 'x = 2\n');
			git('add', '-A');
			git('commit', '-q', '-m', 'edit', '--no-verify');
			expect(changedSampleFindings(root, 'main').map((finding) => finding.file)).toEqual([
				`${SAMPLES}/first.toml`,
			]);
		});
	});
});
