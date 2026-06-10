use nnrp_conformance::wire_conformance::{parse_wire_arguments, write_wire_dry_run_report};

fn main() {
    let args = match parse_wire_arguments(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };

    if let Err(error) = write_wire_dry_run_report(
        &args.suite_manifest,
        &args.target_manifest,
        &args.output,
        &args.selected_case_ids,
    ) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
