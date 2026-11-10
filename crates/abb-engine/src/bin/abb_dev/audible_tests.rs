use super::*;

fn parsed(args: &[&str]) -> Result<Options, String> {
    parse(args.iter().map(|arg| arg.to_string()))
}

#[test]
fn each_command_reads_its_own_flags() {
    let state = PathBuf::from("state");
    assert_eq!(
        parsed(&["sign-in", "--state-dir", "state"]),
        Ok(Options {
            command: Command::SignIn,
            json: false,
            state_dir: state.clone()
        })
    );
    assert_eq!(
        parsed(&["library", "--json", "--state-dir", "state"]),
        Ok(Options {
            command: Command::Library,
            json: true,
            state_dir: state.clone()
        })
    );
    assert_eq!(
        parsed(&[
            "acquire",
            "B0AAAAAAAA",
            "--pdf",
            "B0BBBBBBBB",
            "--state-dir",
            "state",
            "--json"
        ]),
        Ok(Options {
            command: Command::Acquire {
                title_ids: vec!["B0AAAAAAAA".into(), "B0BBBBBBBB".into()],
                pdf: true
            },
            json: true,
            state_dir: state
        })
    );
}

#[test]
fn a_command_without_its_state_folder_or_title_ids_is_refused() {
    for (args, reason) in [
        (&["library"][..], "--state-dir"),
        (&["--state-dir", "state"][..], "audible needs a command"),
        (
            &["acquire", "--state-dir", "state"][..],
            "at least one title id",
        ),
        (
            &["acquire", "B0A", "B0A", "--state-dir", "state"][..],
            "listed twice",
        ),
        (
            &["fetch", "--state-dir", "state"][..],
            "unknown audible command",
        ),
        (
            &["library", "--state-dir"][..],
            "--state-dir needs a folder",
        ),
    ] {
        let error = parsed(args).expect_err("must be refused");
        assert!(error.contains(reason), "{args:?}: {error}");
    }
}

#[test]
fn a_flag_or_id_a_command_does_not_use_is_refused() {
    for (args, reason) in [
        (
            &["sign-in", "--json", "--state-dir", "state"][..],
            "--json does not apply",
        ),
        (
            &["library", "--pdf", "--state-dir", "state"][..],
            "--pdf does not apply",
        ),
        (
            &["library", "B0A", "--state-dir", "state"][..],
            "takes no title ids",
        ),
        (
            &["acquire", "B0A", "--bogus", "--state-dir", "state"][..],
            "unknown option",
        ),
    ] {
        let error = parsed(args).expect_err("must be refused");
        assert!(error.contains(reason), "{args:?}: {error}");
    }
}

#[test]
fn a_pasted_line_is_an_address_unless_it_is_empty_or_only_a_ctrl_d() {
    assert_eq!(
        pasted_address("  https://example.test/landing?code=1\r\n"),
        Some("https://example.test/landing?code=1")
    );
    assert_eq!(pasted_address("\n"), None);
    assert_eq!(pasted_address("   \n"), None);
    assert_eq!(pasted_address("\u{4}"), None);
    assert_eq!(pasted_address("\u{4}\n"), None);
    assert_eq!(pasted_address(" \u{4} \n"), None);
}
