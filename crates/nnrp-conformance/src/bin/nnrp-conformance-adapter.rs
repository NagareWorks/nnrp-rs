use std::{env, path::Path};

use nnrp_conformance::adapter_conformance::{parse_arguments, write_results_report};

fn main() -> Result<(), String> {
    let options = parse_arguments(
        env::args().skip(1),
        env::var("NNRP_CONFORMANCE_ADAPTER_PLAN").ok(),
        env::var("NNRP_CONFORMANCE_ADAPTER_RESULTS").ok(),
    )?;
    write_results_report(
        Path::new(&options.plan_path),
        Path::new(&options.output_path),
    )?;
    Ok(())
}
