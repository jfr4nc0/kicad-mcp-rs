use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("vendored protoc is available");

    compile_tree(Path::new("proto"), &out.join("kicad10"), &protoc);
    compile_tree(
        Path::new("proto/kicad11-preview"),
        &out.join("kicad11"),
        &protoc,
    );

    println!("cargo:rerun-if-changed=proto");
}

fn compile_tree(root: &Path, out: &Path, protoc: &Path) {
    fs::create_dir_all(out).expect("create protobuf output directory");

    let mut files = Vec::new();
    for directory in ["board", "common", "schematic"] {
        collect_protos(&root.join(directory), &mut files);
    }
    files.sort();

    let mut config = prost_build::Config::new();
    config.out_dir(out);
    config.protoc_executable(protoc);
    config
        .compile_protos(&files, &[root])
        .expect("compile KiCad protobuf definitions");
}

fn collect_protos(path: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(path).expect("read protobuf directory") {
        let path = entry.expect("read protobuf entry").path();
        if path.is_dir() {
            collect_protos(&path, files);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "proto")
        {
            files.push(path);
        }
    }
}
