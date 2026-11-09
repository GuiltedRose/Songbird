use std::{env, fs, path::PathBuf};
fn main() {
    println!("cargo:rerun-if-changed=data/cmudict.dict");
    let data = fs::read_to_string("data/cmudict.dict").expect("bundled CMU dictionary");
    assert!(
        data.is_ascii(),
        "dictionary binary search requires ASCII source"
    );
    // Upstream groups pronunciation variants before possessives, which is not
    // lexical byte order. Sort at build time rather than allocating a runtime index.
    let mut lines = data.lines().collect::<Vec<_>>();
    lines.sort_unstable();
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo OUT_DIR")).join("english.dict");
    fs::write(output, lines.join("\n") + "\n").expect("write embedded dictionary");
}
