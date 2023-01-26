fn main() {
    println!("cargo:rustc-link-lib=static=freetype");
    println!("cargo:rustc-link-search=win64");
}
