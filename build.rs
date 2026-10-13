fn main() {
    #[cfg(target_os = "windows")]
    {
        // UACの要求はセキュリティ上必須。マニフェスト埋込み失敗ならビルドを失敗させる。
        println!("cargo:rerun-if-changed=assets/app.rc");
        println!("cargo:rerun-if-changed=assets/app.manifest");
        println!("cargo:rerun-if-changed=assets/app.ico");
        embed_resource::compile("assets/app.rc", embed_resource::NONE)
            .manifest_required()
            .expect("Windows UAC manifest must be embedded");
    }
}
