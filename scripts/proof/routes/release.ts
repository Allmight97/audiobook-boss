import { ProofUsageError, plan } from '../plan';
import { bunStep, cargoStep } from '../steps';
import type { ProofPlan } from '../types';
import { reviewPlan } from './review';

export function releasePlan(args: string[]): ProofPlan {
	const [target = 'package', ...rest] = args;
	if (target !== 'package' || rest.length > 0) {
		throw new ProofUsageError('Usage: bun scripts/proof/runner.ts release [package]');
	}

	return plan(
		'release.package',
		'Release/package proof',
		'release',
		'Run review proof plus app packaging and AAC decoder contract binary.',
		[
			...reviewPlan([]).steps,
			bunStep('tauri-app-package', 'Tauri app packaging', 'run', 'app:build'),
			cargoStep(
				'aac-decoder-contract-binary',
				'AAC decoder contract binary',
				'run',
				'--manifest-path',
				'src-tauri/Cargo.toml',
				'--bin',
				'verify_aac_decoder_contract',
				'--quiet',
			),
		],
	);
}
