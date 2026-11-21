fn main() {
    println!("cargo:rerun-if-env-changed=RAZE_RESOURCE");
    if let Ok(resource) = std::env::var("RAZE_RESOURCE") {
        println!("cargo:rustc-link-arg-bin=Raze-Macro-Setup={resource}");
    }
}
