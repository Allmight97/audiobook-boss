//! Every collision kind under every policy: what the plan does, what review
//! lets through, and that publishing it never changes a file it was not
//! planned to replace. A slip in a source-overlap row replaces the user's
//! source book.

use super::{
    commit_output_artifact, enforce_output_plan_review, CollisionPolicy, OutputCollisionKind,
    OutputCommitRequest, OutputKind, OutputPlanLedger, OutputPlanReview, PlannedOutputAction,
};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const POLICIES: [CollisionPolicy; 4] = [
    CollisionPolicy::Fail,
    CollisionPolicy::ReplaceExisting,
    CollisionPolicy::RenameNew,
    CollisionPolicy::SkipExisting,
];
const NEW_BOOK: &[u8] = b"the book being exported";
const OLD_BOOK: &[u8] = b"a book already on disk";

struct Scenario {
    name: &'static str,
    kind: Option<OutputCollisionKind>,
    requested: PathBuf,
    sources: Vec<PathBuf>,
    protected: Vec<PathBuf>,
    ledger: OutputPlanLedger,
    actions: [PlannedOutputAction; 4],
}

impl Scenario {
    fn hard_block(&self) -> bool {
        matches!(
            self.kind,
            Some(
                OutputCollisionKind::SourceDestinationOverlap
                    | OutputCollisionKind::CanonicalPathOverlap
            )
        )
    }
}

fn case_insensitive_folder(folder: &Path) -> bool {
    let probe = folder.join("Probe.tmp");
    fs::write(&probe, b"probe").expect("probe file");
    let insensitive = folder.join("probe.tmp").exists();
    fs::remove_file(&probe).expect("remove probe");
    insensitive
}

fn claimed_first(ledger: &mut OutputPlanLedger, path: &Path) {
    let first = ledger
        .resolve(path, OutputKind::Final, CollisionPolicy::Fail, &[])
        .expect("first output of the run");
    assert_eq!(first.action, PlannedOutputAction::Write);
}

fn scenario(
    name: &'static str,
    kind: Option<OutputCollisionKind>,
    requested: PathBuf,
    actions: [PlannedOutputAction; 4],
) -> Scenario {
    Scenario {
        name,
        kind,
        requested,
        sources: vec![],
        protected: vec![],
        ledger: OutputPlanLedger::new(),
        actions,
    }
}

fn file_in(folder: &Path, name: &str) -> PathBuf {
    fs::create_dir_all(folder).expect("folder");
    let path = folder.join(name);
    fs::write(&path, OLD_BOOK).expect("book on disk");
    path
}

fn on_disk_scenarios(folder: &Path) -> Vec<Scenario> {
    use OutputCollisionKind as Kind;
    use PlannedOutputAction::{RenameNew, ReplaceExisting, ReviewRequired, SkipExisting, Write};

    let free = scenario("no collision", None, folder.join("book.m4b"), [Write; 4]);

    let existing = file_in(&folder.join("existing"), "book.m4b");
    let mut existing_file = scenario(
        "existing file",
        Some(Kind::ExistingFile),
        existing.clone(),
        [ReviewRequired, ReplaceExisting, RenameNew, SkipExisting],
    );
    existing_file.protected = vec![existing];

    let cased = folder.join("cased");
    let other_case = file_in(&cased, "Book.m4b");
    let disk_kind = if case_insensitive_folder(&cased) {
        Kind::ExistingFile
    } else {
        Kind::CaseInsensitiveMatch
    };
    let mut cased_file = scenario(
        "a file on disk in other letter case",
        Some(disk_kind),
        cased.join("book.m4b"),
        [ReviewRequired, ReplaceExisting, RenameNew, SkipExisting],
    );
    cased_file.protected = vec![other_case];

    vec![free, existing_file, cased_file]
}

fn claimed_scenarios(folder: &Path) -> Vec<Scenario> {
    use OutputCollisionKind as Kind;
    use PlannedOutputAction::{RenameNew, ReviewRequired, SkipExisting};

    let actions = [ReviewRequired, RenameNew, RenameNew, SkipExisting];
    let duplicate = folder.join("duplicate").join("book.m4b");
    let mut run_duplicate = scenario(
        "another output of the run",
        Some(Kind::BatchDuplicate),
        duplicate.clone(),
        actions,
    );
    claimed_first(&mut run_duplicate.ledger, &duplicate);

    let claimed = folder.join("claimed").join("Book.m4b");
    let mut run_case_duplicate = scenario(
        "another output of the run in other letter case",
        Some(Kind::CaseInsensitiveMatch),
        folder.join("claimed").join("book.m4b"),
        actions,
    );
    claimed_first(&mut run_case_duplicate.ledger, &claimed);

    vec![run_duplicate, run_case_duplicate]
}

fn source_scenarios(folder: &Path) -> Vec<Scenario> {
    use OutputCollisionKind as Kind;

    let source = file_in(&folder.join("sources"), "book.m4b");
    let mut itself = scenario(
        "the source book itself",
        Some(Kind::SourceDestinationOverlap),
        source.clone(),
        [PlannedOutputAction::ReviewRequired; 4],
    );
    itself.sources = vec![source.clone()];
    itself.protected = vec![source];

    let real = folder.join("real");
    let linked_source = file_in(&real, "book.m4b");
    std::os::unix::fs::symlink(&real, folder.join("link")).expect("link to the source folder");
    let mut through_link = scenario(
        "the source book through a link",
        Some(Kind::CanonicalPathOverlap),
        folder.join("link").join("book.m4b"),
        [PlannedOutputAction::ReviewRequired; 4],
    );
    through_link.sources = vec![linked_source.clone()];
    through_link.protected = vec![linked_source];

    vec![itself, through_link]
}

fn scenarios(folder: &Path) -> Vec<Scenario> {
    let mut all = on_disk_scenarios(folder);
    all.extend(claimed_scenarios(folder));
    all.extend(source_scenarios(folder));
    all
}

fn same_name_ignoring_case(left: &Path, right: &Path) -> bool {
    left.to_string_lossy().to_lowercase() == right.to_string_lossy().to_lowercase()
}

fn publish(scenario: &Scenario, plan: &super::ResolvedOutputPlan, label: &str, folder: &Path) {
    let staged = folder.join("work").join("staged.m4b");
    fs::create_dir_all(staged.parent().expect("work folder")).expect("work folder");
    fs::write(&staged, NEW_BOOK).expect("staged book");
    let before: Vec<_> = scenario
        .protected
        .iter()
        .map(|path| fs::read(path).expect("protected book"))
        .collect();
    let existed = plan.resolved_path.exists();
    let committed = commit_output_artifact(
        OutputCommitRequest::new(&plan.resolved_path, plan.action),
        staged.clone(),
    );
    match plan.action {
        PlannedOutputAction::ReviewRequired | PlannedOutputAction::SkipExisting => {
            assert!(committed.is_err(), "{label}: nothing is published");
            assert_eq!(
                existed,
                plan.resolved_path.exists(),
                "{label}: destination state"
            );
        }
        PlannedOutputAction::Write | PlannedOutputAction::RenameNew => {
            assert!(!existed, "{label}: a new name was free");
            committed.unwrap_or_else(|error| panic!("{label}: {error}"));
            assert_eq!(fs::read(&plan.resolved_path).expect("published"), NEW_BOOK);
        }
        PlannedOutputAction::ReplaceExisting => {
            committed.unwrap_or_else(|error| panic!("{label}: {error}"));
            assert_eq!(fs::read(&plan.resolved_path).expect("replaced"), NEW_BOOK);
        }
    }
    for (path, bytes) in scenario.protected.iter().zip(before) {
        let replaced = plan.action == PlannedOutputAction::ReplaceExisting
            && same_name_ignoring_case(path, &plan.resolved_path);
        if !replaced {
            assert_eq!(
                fs::read(path).expect("protected book survives"),
                bytes,
                "{label}: {} is untouched",
                path.display()
            );
        }
    }
}

#[test]
fn every_collision_kind_under_every_policy_plans_reviews_and_publishes_as_pinned() {
    for (index, policy) in POLICIES.into_iter().enumerate() {
        let root = TempDir::new().expect("scenario root");
        for mut scenario in scenarios(root.path()) {
            let label = format!("{} under {policy:?}", scenario.name);
            let plan = scenario
                .ledger
                .resolve(
                    &scenario.requested,
                    OutputKind::Final,
                    policy,
                    &scenario.sources,
                )
                .unwrap_or_else(|error| panic!("{label}: {error}"));

            assert_eq!(plan.action, scenario.actions[index], "{label}: action");
            assert_eq!(
                plan.collision.as_ref().map(|collision| collision.kind),
                scenario.kind,
                "{label}: kind"
            );
            let free_name = scenario.requested.with_file_name("book-1.m4b");
            let wants_free_name = scenario.kind.is_some() && !scenario.hard_block();
            assert_eq!(
                plan.rename_candidate,
                wants_free_name.then(|| free_name.clone()),
                "{label}: rename candidate"
            );
            let expected_path = if plan.action == PlannedOutputAction::RenameNew {
                free_name
            } else {
                scenario.requested.clone()
            };
            assert_eq!(plan.resolved_path, expected_path, "{label}: resolved path");

            let public = plan.to_public(0, &scenario.requested);
            match plan.action {
                PlannedOutputAction::ReviewRequired => {
                    let review = public.review.expect("a review requirement");
                    assert_eq!(review.can_proceed, !scenario.hard_block(), "{label}");
                }
                _ => assert_eq!(public.review, None, "{label}: no review needed"),
            }

            let reviewed = enforce_output_plan_review(
                OutputPlanReview {
                    expected_signature: Some("sig"),
                    current_signature: "sig",
                    collision_policy: policy,
                },
                [&plan],
            );
            if scenario.hard_block() {
                let error = reviewed.expect_err(&format!("{label}: a reviewed hard block"));
                assert!(
                    error.to_string().contains("targets an input source file"),
                    "{label}: {error}"
                );
            } else {
                reviewed.unwrap_or_else(|error| panic!("{label}: {error}"));
            }
            let unreviewed = enforce_output_plan_review(
                OutputPlanReview {
                    expected_signature: None,
                    current_signature: "sig",
                    collision_policy: policy,
                },
                [&plan],
            );
            assert_eq!(
                unreviewed.is_ok(),
                scenario.kind.is_none() && policy == CollisionPolicy::Fail,
                "{label}: without a review only a collision-free plan under Fail proceeds"
            );

            publish(&scenario, &plan, &label, root.path());
        }
    }
}
