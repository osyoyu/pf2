fn main() {
    cc::Build::new().file("src/siginfo_t.c").compile("ccode");
}
