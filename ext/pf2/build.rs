use std::env;

fn main() {
    cc::Build::new().file("src/siginfo_t.c").compile("ccode");
    cc::Build::new()
        .flag(format!("-I{}", env::var("DEP_RB_RBCONFIG_RUBYHDRDIR").unwrap()).as_str())
        .flag(format!("-I{}", env::var("DEP_RB_RBCONFIG_RUBYARCHHDRDIR").unwrap()).as_str())
        .file("src/ruby_c_api_helper.c")
        .compile("rubyhelper");
}
