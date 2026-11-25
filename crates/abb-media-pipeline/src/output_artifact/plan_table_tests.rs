use super::{
    commit_output_artifact, enforce_output_plan_review, CollisionPolicy, OutputCollisionKind,
    OutputCommitRequest, OutputKind, OutputPlanLedger, OutputPlanReview, PlannedOutputAction,
};
use abb_output_artifact_core::RenameOption;
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
    replaced: Option<PathBuf>,
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
        replaced: None,
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
    cased_file.protected = vec![other_case.clone()];
    cased_file.replaced = Some(other_case);

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

    let cased_source = file_in(&folder.join("cased-source"), "Book.m4b");
    let overlap_kind = if case_insensitive_folder(&folder.join("cased-source")) {
        Kind::CanonicalPathOverlap
    } else {
        Kind::SourceDestinationOverlap
    };
    let mut other_case = scenario(
        "the source book in other letter case",
        Some(overlap_kind),
        folder.join("cased-source").join("book.m4b"),
        [PlannedOutputAction::ReviewRequired; 4],
    );
    other_case.sources = vec![cased_source.clone()];
    other_case.protected = vec![cased_source];

    vec![itself, through_link, other_case]
}

fn scenarios(folder: &Path) -> Vec<Scenario> {
    let mut all = on_disk_scenarios(folder);
    all.extend(claimed_scenarios(folder));
    all.extend(source_scenarios(folder));
    all
}

fn same_file(left: &Path, right: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (fs::metadata(left), fs::metadata(right)) {
        (Ok(left), Ok(right)) => left.dev() == right.dev() && left.ino() == right.ino(),
        _ => false,
    }
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
            let folder = plan.resolved_path.parent().expect("output folder");
            let names: Vec<_> = fs::read_dir(folder)
                .expect("output folder")
                .map(|entry| entry.expect("entry").file_name())
                .collect();
            assert_eq!(
                names,
                [plan.resolved_path.file_name().expect("name")],
                "{label}: the replaced file keeps its name"
            );
        }
    }
    for (path, bytes) in scenario.protected.iter().zip(before) {
        let replaced = plan.action == PlannedOutputAction::ReplaceExisting
            && same_file(path, &plan.resolved_path);
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
            let expected_rename = if wants_free_name {
                RenameOption::Free(free_name.clone())
            } else {
                RenameOption::Unneeded
            };
            assert_eq!(plan.rename, expected_rename, "{label}: rename option");
            let expected_path = match plan.action {
                PlannedOutputAction::RenameNew => free_name,
                PlannedOutputAction::ReplaceExisting => scenario
                    .replaced
                    .clone()
                    .unwrap_or_else(|| scenario.requested.clone()),
                _ => scenario.requested.clone(),
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

#[test]
fn exhausted_rename_suffixes_refuse_only_rename() {
    let root = TempDir::new().expect("scenario root");
    let requested = file_in(root.path(), "book.m4b");
    for suffix in 1..=99 {
        file_in(root.path(), &format!("book-{suffix}.m4b"));
    }
    let expected = [
        Some(PlannedOutputAction::ReviewRequired),
        Some(PlannedOutputAction::ReplaceExisting),
        None,
        Some(PlannedOutputAction::SkipExisting),
    ];
    for (policy, expected) in POLICIES.into_iter().zip(expected) {
        let planned = OutputPlanLedger::new().resolve(&requested, OutputKind::Final, policy, &[]);
        match expected {
            Some(action) => assert_eq!(
                planned.map(|plan| plan.action).ok(),
                Some(action),
                "{policy:?}"
            ),
            None => {
                let error = planned.expect_err("no free renamed name");
                assert!(error.to_string().contains("book-99.m4b"), "{error}");
            }
        }
    }
}
