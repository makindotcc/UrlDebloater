fn main() {
    #[cfg(windows)]
    embed_resource::compile("./wix/urldebloater.rc", embed_resource::NONE);
}
