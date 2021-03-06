use crate::builtins::InternalCommandContext;
use crate::exec::ExitStatus;
use std::io::Write;

pub fn command(ctx: &mut InternalCommandContext) -> ExitStatus {
    if let Some(filepath) = ctx.argv.get(1) {
        ctx.isolate.run_file(std::path::PathBuf::from(&filepath))
    } else {
        writeln!(ctx.stderr, "nsh: source: filename argument required").ok();
        ctx.stderr.flush().ok();
        ExitStatus::ExitedWith(0)
    }
}
