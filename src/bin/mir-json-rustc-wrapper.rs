//! Entry point for use with `cargo crux-test` / `RUSTC_WRAPPER`.  This will export MIR (like the
//! main `mir-json` binary), and if this is a top-level build, it will also link in all libraries
//! as specified by `--extern` and/or `#![no_std]` and run `mir-verifier` on the result.
#![feature(rustc_private)]

extern crate rustc;
extern crate rustc_codegen_utils;
extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_metadata;
extern crate getopts;
extern crate syntax;
extern crate rustc_errors;
extern crate rustc_target;

extern crate mir_json;

use mir_json::analyz;
use mir_json::link;
use rustc::session::Session;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::{Compiler, Config};
use rustc::session::config::{self, Input, ErrorOutputType};
use rustc_codegen_utils::codegen_backend::CodegenBackend;
use rustc_metadata::cstore::CStore;
use rustc_target::spec::PanicStrategy;
use syntax::ast;
use std::env;
use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::iter;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;


/// Driver callbacks that get the output filename and then stop compilation.  This is used to get
/// the path of the test executable when compiling in `--test` mode.
#[derive(Default)]
struct GetOutputPathCallbacks {
    output_path: Option<PathBuf>,
}

impl rustc_driver::Callbacks for GetOutputPathCallbacks {
    fn after_analysis<'tcx>(
        &mut self,
        compiler: &Compiler,
    ) -> Compilation {
        let sess = compiler.session();
        let crate_name = compiler.crate_name().unwrap().peek();
        let outputs = compiler.prepare_outputs().unwrap().peek();
        self.output_path = Some(rustc_codegen_utils::link::out_filename(
            sess,
            sess.crate_types.get().first().unwrap().clone(),
            &outputs,
            &crate_name,
        ));
        Compilation::Stop
    }
}

fn get_output_path(args: &[String]) -> PathBuf {
    let mut callbacks = GetOutputPathCallbacks::default();
    rustc_driver::run_compiler(
        &args,
        &mut callbacks,
        None,
        None,
    ).unwrap();
    callbacks.output_path.unwrap()
}


#[derive(Debug, Default)]
struct MirJsonCallbacks {
    analysis_data: Option<analyz::AnalysisData<()>>,
}

impl rustc_driver::Callbacks for MirJsonCallbacks {
    /// Called after analysis. Return value instructs the compiler whether to
    /// continue the compilation afterwards (defaults to `Compilation::Continue`)
    fn after_analysis(&mut self, compiler: &Compiler) -> Compilation {
        self.analysis_data = analyz::analyze(compiler).unwrap();
        Compilation::Continue
    }
}

fn link_mirs(main_path: PathBuf, extern_paths: &[PathBuf], out_path: &Path) {
    let mut inputs = iter::once(&main_path).chain(extern_paths.iter())
        .map(File::open)
        .collect::<io::Result<Vec<_>>>().unwrap();
    let mut output = io::BufWriter::new(File::create(out_path).unwrap());
    link::link_crates(&mut inputs, output).unwrap();
}

fn write_test_script(script_path: &Path, json_path: &Path) -> io::Result<()> {
    let json_name = json_path.file_name().unwrap().to_str().unwrap();
    let mut f = OpenOptions::new().write(true).create(true).truncate(true)
        .mode(0o755).open(script_path)?;
    writeln!(f, "#!/bin/sh")?;
    writeln!(f, r#"exec crux-mir --assert-false-on-error "$(dirname "$0")"/'{}'"#, json_name)?;
    Ok(())
}

fn go() {
    // First arg is the name of the `rustc` binary that cargo means to invoke, which we ignore.
    let mut args: Vec<String> = std::env::args().skip(1).collect();

    // XXX big hack: We need to use normal rustc (with its normal libs) for `build.rs` scripts,
    // since our custom libs aren't actually functional.  To distinguish `build.rs` and `build.rs`
    // dependencies from other compilation jobs, we pass `--target x86_64-unknown-linux-gnu` to
    // `cargo`.  This makes cargo use cross-compilation mode, even though the host and target
    // triples are the same.  In that mode, it passes the provided `--target` through to target
    // jobs, and omit `--target` for host jobs.  So if `--target` is missing, this is a `build.rs`
    // build, and we should `exec` the real Rust compiler instead of doing our normal thing.
    if args.iter().position(|s| s == "--target").is_none() {
        let rustc = &args[0];
        let args = &args[1..];
        eprintln!("this is a host build - exec {:?} {:?}", rustc, args);
        let e = Command::new(rustc)
            .args(args)
            .exec();
        unreachable!("exec failed: {:?}", e);
    }

    // All build steps need `--cfg crux` and library paths.
    args.push("--cfg".into());
    args.push("crux".into());

    if let Ok(s) = env::var("CRUX_RUST_LIBRARY_PATH") {
        args.push("-L".into());
        args.push(s);
    }


    let test_idx = match args.iter().position(|s| s == "--test") {
        None => {
            eprintln!("normal build - {:?}", args);
            // This is a normal, non-test build.  Just run the build, generating a `.mir` file
            // alongside the normal output.
            rustc_driver::run_compiler(
                &args,
                &mut MirJsonCallbacks::default(),
                None,
                None,
            ).unwrap();
            return;
        },
        Some(x) => x,
    };

    // This is a `--test` build.  We need to build the `.mir`s for this crate, link with `.mir`s
    // for all its dependencies, and produce a test script (in place of the test binary expected by
    // cargo) that will run `crux-mir` on the linked JSON file.

    // We're still using the original args (with only a few modifications), so the output path
    // should be the path of the test binary.
    eprintln!("test build - extract output path - {:?}", args);
    let test_path = get_output_path(&args);

    args.remove(test_idx);

    args.push("--cfg".into());
    args.push("crux_top_level".into());

    // Cargo doesn't pass a crate type for `--test` builds.  We fill in a reasonable default.
    args.push("--crate-type".into());
    args.push("rlib".into());

    eprintln!("test build - {:?}", args);

    // Now run the compiler.  Note we rely on cargo providing different metadata and extra-filename
    // strings to prevent collisions between this build's `.mir` output and other builds of the
    // same crate.
    let mut callbacks = MirJsonCallbacks::default();
    rustc_driver::run_compiler(
        &args,
        &mut callbacks,
        None,
        None,
    ).unwrap();
    let data = callbacks.analysis_data
        .expect("failed to find main MIR path");

    let json_path = test_path.with_extension(".linked-mir.json");
    eprintln!("linking {} mir files into {}", 1 + data.extern_mir_paths.len(), json_path.display());
    eprintln!(
        "  inputs: {}{}",
        data.mir_path.display(),
        data.extern_mir_paths.iter().map(|x| format!(" {}", x.display())).collect::<String>(),
    );
    link_mirs(data.mir_path, &data.extern_mir_paths, &json_path);

    write_test_script(&test_path, &json_path).unwrap();
    eprintln!("generated test script {}", test_path.display());
}

fn main() {
    go();
}
