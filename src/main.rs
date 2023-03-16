#![feature(rustc_private, box_patterns)]

extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_mir_build;
extern crate rustc_session;

use std::process::Command;

use rustc_data_structures::steal::Steal;
use rustc_hir::def_id::LocalDefId;

use rustc_middle::{
  mir::{
    interpret::{ConstValue, Scalar},
    pretty::write_mir_fn,
    BasicBlock, Body, Constant, ConstantKind, Operand, Rvalue, StatementKind,
  },
  ty::{
    self,
    query::{ExternProviders, Providers},
    TyCtxt,
  },
};

fn mir_built(tcx: TyCtxt<'_>, def: ty::WithOptConstParam<LocalDefId>) -> &Steal<Body<'_>> {
  // Get the default body provided by rustc
  let mut providers = Providers::default();
  rustc_mir_build::provide(&mut providers);
  let body = (providers.mir_built)(tcx, def);

  // Take ownership of the body
  let mut body = body.steal();

  // Print out the body for debugging
  let mut buffer = Vec::new();
  write_mir_fn(tcx, &body, &mut |_, _| Ok(()), &mut buffer).unwrap();
  println!("{}", String::from_utf8(buffer).unwrap());

  // Perform some manipulation to the body
  let bb0 = &mut body.basic_blocks_mut()[BasicBlock::from_usize(0)];
  let val = bb0
    .statements
    .iter_mut()
    .find_map(|stmt| match &mut stmt.kind {
      StatementKind::Assign(box (
        lhs,
        Rvalue::Use(Operand::Constant(box Constant {
          literal: ConstantKind::Val(ConstValue::Scalar(val), _),
          ..
        })),
      )) => {
        if lhs.local.as_usize() == 1 {
          Some(val)
        } else {
          None
        }
      }
      _ => None,
    })
    .unwrap();
  eprintln!("{val:#?}");
  *val = Scalar::from_i32(1234);

  // Repackage the body into a Steal<_>
  tcx.alloc_steal_mir(body)
}

fn override_queries(
  _session: &rustc_session::Session,
  local: &mut Providers,
  _external: &mut ExternProviders,
) {
  local.mir_built = mir_built;
}
struct Callbacks;
impl rustc_driver::Callbacks for Callbacks {
  fn config(&mut self, config: &mut rustc_interface::interface::Config) {
    config.override_queries = Some(override_queries);
  }
}

fn main() {
  // Get the sysroot so rustc can find libstd
  let print_sysroot = Command::new("rustc")
    .args(["--print", "sysroot"])
    .output()
    .unwrap()
    .stdout;
  let sysroot = String::from_utf8(print_sysroot).unwrap().trim().to_owned();

  let mut args = std::env::args().collect::<Vec<_>>();
  args.extend(["--sysroot".into(), sysroot, "--emit=llvm-ir".into()]);

  // Run rustc with the given arguments
  let mut callbacks = Callbacks;
  rustc_driver::catch_fatal_errors(|| {
    rustc_driver::RunCompiler::new(&args, &mut callbacks)
      .run()
      .unwrap()
  })
  .unwrap();
}
