fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let mut res = winres::WindowsResource::new();
        // assets/icon.ico 존재 시 아이콘 임베드
        if std::path::Path::new("assets/icon.ico").exists() {
            res.set_icon("assets/icon.ico");
        }
        res.set("ProductName", "BootReady Core");
        res.set("FileDescription", "BootReady Background Monitor");
        res.set("LegalCopyright", "Copyright 2026 BootReady");
        // GUI 앱이 아닌 콘솔 서브시스템 (트레이 전용이므로 windows 서브시스템으로 변경 가능)
        res.set_manifest(r#"
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true</dpiAware>
    </windowsSettings>
  </application>
</assembly>
"#);
        res.compile().unwrap_or_else(|e| eprintln!("winres warning: {e}"));
    }

    // Windows 서브시스템 설정 (콘솔 창 숨김)
    println!("cargo:rustc-link-arg=/SUBSYSTEM:WINDOWS");
    println!("cargo:rustc-link-arg=/ENTRY:mainCRTStartup");
}
