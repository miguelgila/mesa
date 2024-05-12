// use std::env;
// use std::path::PathBuf;
//
// fn main() {
//     let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
//
//     // Step 1, let's generate the `hello.h` file automatically.
//     cbindgen::Builder::new()
//         .with_crate(&manifest_dir)
//         .with_language(cbindgen::Language::C)
//         .generate()
//         .unwrap()
//         .write_to_file("libmesa.h");
//
//     // Step 2, let's set the `CFLAGS` and the `LDFLAGS` variables.
//     let include_dir = manifest_dir.clone();
//     let mut shared_object_dir = PathBuf::from(manifest_dir);
//     shared_object_dir.push("target");
//     shared_object_dir.push(env::var("PROFILE").unwrap());
//     let shared_object_dir = shared_object_dir.as_path().to_string_lossy();
//
//     println!(
//         "cargo:rustc-env=INLINE_C_RS_CFLAGS=-I{I} -L{L} -D_DEBUG -D_CRT_SECURE_NO_WARNINGS",
//         I = include_dir,
//         L = shared_object_dir,
//     );
//
//     println!("cargo:rustc-link-arg=-Clink-arg=-undefined -Clink-arg=dynamic_lookup -Clink-args=-rdynamic");
//
//     // println!(
//     //     "cargo:rustc-env=INLINE_C_RS_LDFLAGS={shared_object_dir}/{lib}",
//     //     shared_object_dir = shared_object_dir,
//     //     lib = "libhello.dylib",
//     // );
// }


use std::env;
use std::path::PathBuf;

// #[cfg(feature = "ffi")]
fn main() {
    const LIB_NAME: &str = "libmesa.a";

    protobuf_codegen::Codegen::new()
        .cargo_out_dir("protos")
        .include("src")
        .input("src/protos/terraform.proto")
        .run_from_script();

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let include_dir = manifest_dir.clone();
    let mut shared_object_dir = PathBuf::from(manifest_dir);
    shared_object_dir.push("target");
    shared_object_dir.push(env::var("PROFILE").unwrap());
    let shared_object_dir = shared_object_dir.as_path().to_string_lossy();

    println!(
        "cargo:rustc-env=INLINE_C_RS_CFLAGS=-I{I} -L{L} -D_DEBUG -D_CRT_SECURE_NO_WARNINGS -ldl -lmesa",
        I = include_dir,
        L = shared_object_dir,
    );

    println!(
        "cargo:rustc-env=INLINE_C_RS_LDFLAGS={shared_object_dir}/{lib}",
        shared_object_dir = shared_object_dir,
        lib = LIB_NAME,
    );
}

// #[cfg(not(feature = "ffi"))]
// fn main() {}