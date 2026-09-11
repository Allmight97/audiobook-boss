import { createHash } from 'node:crypto';
import { readFileSync, realpathSync } from 'node:fs';
import path from 'node:path';

// The checkout owns its development settings, WebView state, and temporary jobs.
// Keep the identity stable across branch changes and distinct from installed apps.
export function withDevelopmentIdentity(repoRoot: string, args: string[]): string[] {
	const boundary = args.indexOf('--');
	const cliArgs = boundary < 0 ? args : args.slice(0, boundary);
	const command = cliArgs.find((arg) => !arg.startsWith('-'));
	const debugBuild = command === 'build' && (cliArgs.includes('--debug') || cliArgs.includes('-d'));
	if (command !== 'dev' && !debugBuild) return args;

	const checkout = realpathSync(repoRoot);
	const config: { identifier: string; productName: string } = JSON.parse(
		readFileSync(path.join(checkout, 'src-tauri/tauri.conf.json'), 'utf8'),
	);
	const checkoutId = createHash('sha256').update(checkout).digest('hex').slice(0, 12);
	const overlay = {
		identifier: `${config.identifier}.dev.${checkoutId}`,
		productName: `${config.productName} Dev ${checkoutId.slice(0, 6)}`,
	};
	return [
		...cliArgs,
		'--config',
		JSON.stringify(overlay),
		...(boundary < 0 ? [] : args.slice(boundary)),
	];
}

if (import.meta.main) {
	const args = withDevelopmentIdentity(process.cwd(), process.argv.slice(2));
	const { run } = await import('@tauri-apps/cli');
	await run(args, 'bun run tauri').catch((error: unknown) => {
		console.error(error instanceof Error ? error.message : error);
		process.exitCode = 1;
	});
}
