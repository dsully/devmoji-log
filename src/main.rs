mod commit;

use clap::Parser;
use jiff::Zoned;
use jiff::fmt::friendly::{Designator, Spacing, SpanPrinter};

use crate::commit::Commit;

#[derive(Debug, clap::Parser)]
#[clap(
    name = "devmoji-log",
    about = "Show recent Git activity with conventional commit parsing + devmoji ✨"
)]
struct Cli {
    #[clap(
        short,
        long,
        value_name = "number",
        default_value_t = 5,
        help = "Number of commits to retrieve"
    )]
    count: usize,
}

pub fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let now = Zoned::now();
    let commits = Commit::last_n_commits(cli.count)?;

    if !commits.is_empty() {
        //
        let printer = SpanPrinter::new()
            .direction(jiff::fmt::friendly::Direction::Suffix)
            .spacing(Spacing::BetweenUnitsAndDesignators)
            .comma_after_designator(true)
            .designator(Designator::Verbose);

        println!("  ## Recent Activity");
        println!();

        for c in commits {
            println!("  * {} {}", c.id(), c.format(&now, &printer)?);
        }

        println!();
    }

    Ok(())
}
