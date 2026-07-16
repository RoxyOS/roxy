#[macro_export]
macro_rules! cmd {
    ($command:literal) => {{
        let shell = xshell::Shell::new()?;
        xshell::cmd!(&shell, $command).run()
    }};
}
