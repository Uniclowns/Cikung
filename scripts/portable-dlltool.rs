//! Adapt rustc's dlltool request to Rust's bundled LLVM import-library writer.
//! Used only by the isolated Windows GNU build when GNU assembler is absent.
use std::{
    env, fs,
    path::PathBuf,
    process::{exit, Command},
};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let value = |flag: &str| {
        args.windows(2)
            .find(|pair| pair[0] == flag)
            .map(|pair| pair[1].clone())
            .expect("rustc dlltool argument")
    };
    let input = PathBuf::from(value("-d"));
    let output = value("-l");
    let library = value("-D");
    let mut definition = fs::read_to_string(&input).expect("read import definition");
    if !definition
        .lines()
        .any(|line| line.trim_start().to_uppercase().starts_with("LIBRARY"))
    {
        definition = format!("LIBRARY \"{library}\"\n{definition}");
    }
    let temporary = input.with_extension("llvm.def");
    fs::write(&temporary, definition).expect("write LLVM import definition");
    let tool = env::var_os("CIKUNG_LLVM_LINKER").expect("CIKUNG_LLVM_LINKER");
    let status = Command::new(tool)
        .args(["-flavor", "link", "/lib", "/machine:x64"])
        .arg(format!("/def:{}", temporary.display()))
        .arg(format!("/out:{output}"))
        .status()
        .expect("run bundled LLVM import-library writer");
    let _ = fs::remove_file(temporary);
    exit(status.code().unwrap_or(1));
}
