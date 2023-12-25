const BASE_URL_KEY: &str = "VIDSTREAMING_RS_BASE_URL";
const DEFAULT_BASE_URL_VALUE: &str = "https://embtaku.pro/";

fn main() {
    let base_url_value = std::env::var(BASE_URL_KEY);
    println!("cargo:rerun-if-env-changed={BASE_URL_KEY}");
    let base_url_value = match base_url_value.as_ref() {
        Ok(v) => v,
        Err(std::env::VarError::NotPresent) => DEFAULT_BASE_URL_VALUE,
        Err(std::env::VarError::NotUnicode(_)) => {
            panic!("the environment variable \"{BASE_URL_KEY}\" is not valid unicode");
        }
    };

    println!("cargo:rustc-env={BASE_URL_KEY}={base_url_value}");
}
