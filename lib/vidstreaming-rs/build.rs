fn main() {
    let base_url_key = "VIDSTREAMING_RS_BASE_URL";
    let base_url_value = "https://gogohd.net/";
    println!("cargo:rustc-env={}={}", base_url_key, base_url_value,);
}
