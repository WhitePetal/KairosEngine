use crate::ecs::error::KairosError;

#[test]
#[cfg(not(miri))] // miri backtraces are weird
#[cfg(not(windows))] // the windows backtrace in this context is ... unhelpful and not worth testing
fn filtered_backtrace_test() {
    fn i_fail() -> crate::ecs::error::Result {
        let _: usize = "I am not a number".parse()?;
        Ok(())
    }

    let capture_backtrace = std::env::var_os("RUST_BACKTRACE");

    if capture_backtrace.is_none() || capture_backtrace.clone().is_some_and(|s| s == "0") {
        panic!(
            "This test only works if rust backtraces are enabled. Value set was {capture_backtrace:?}. Please set RUST_BACKTRACE to any value other than 0 and run again."
        )
    }

    let error = i_fail().err().unwrap();
    let debug_message = std::format!("{error:?}");
    let mut lines = debug_message.lines().peekable();
    assert_eq!(
        "ParseIntError { kind: InvalidDigit }",
        lines.next().unwrap()
    );

    // On mac backtraces can start with Backtrace::create
    // Rust 1.95 changed the format to use angle brackets: <std::backtrace::Backtrace>::create
    let mut skip = false;
    if let Some(line) = lines.peek()
        && (line[6..] == *"std::backtrace::Backtrace::create"
            || line[6..] == *"<std::backtrace::Backtrace>::create")
    {
        skip = true;
    }

    if skip {
        lines.next().unwrap();
    }

    let expected_lines = std::vec![
        "bevy_ecs::error::bevy_error::tests::filtered_backtrace_test::i_fail",
        "bevy_ecs::error::bevy_error::tests::filtered_backtrace_test",
        "bevy_ecs::error::bevy_error::tests::filtered_backtrace_test::{{closure}}",
        "core::ops::function::FnOnce::call_once",
    ];

    for expected in expected_lines {
        let line = lines.next().unwrap();
        assert_eq!(&line[6..], expected);
        let mut skip = false;
        if let Some(line) = lines.peek()
            && line.starts_with("             at")
        {
            skip = true;
        }

        if skip {
            lines.next().unwrap();
        }
    }

    // on linux there is a second call_once
    let mut skip = false;
    if let Some(line) = lines.peek()
        && &line[6..] == "core::ops::function::FnOnce::call_once"
    {
        skip = true;
    }

    if skip {
        lines.next().unwrap();
    }
    let mut skip = false;
    if let Some(line) = lines.peek()
        && line.starts_with("             at")
    {
        skip = true;
    }

    if skip {
        lines.next().unwrap();
    }
    assert_eq!(super::FILTER_MESSAGE, lines.next().unwrap());
    assert!(lines.next().is_none());
}

#[test]
fn downcasting() {
    #[derive(Debug, PartialEq)]
    struct Fun(i32);

    impl core::fmt::Display for Fun {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            core::fmt::Debug::fmt(&self, f)
        }
    }
    impl core::error::Error for Fun {}

    let new_error = KairosError::new(crate::ecs::error::Severity::Debug, Fun(1));

    assert!(new_error.is::<Fun>());
    assert_eq!(new_error.downcast_ref::<Fun>(), Some(&Fun(1)));
}
