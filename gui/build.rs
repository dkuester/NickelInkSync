//! On Windows: the app icon (`packaging/windows/icon.ico`, made from
//! `packaging/icon.svg` and, up to 32 pixels, `icon-small.svg`) and the
//! version go into the executable, so Explorer shows them. Elsewhere
//! nothing happens.

fn main() {
  println!("cargo:rerun-if-changed=../packaging/windows/icon.ico");
  if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
    return;
  }
  let mut res = winresource::WindowsResource::new();
  res
    .set_icon("../packaging/windows/icon.ico")
    .set("ProductName", "NickelInkSync")
    .set("FileDescription", "NickelInkSync");
  // Without the resource compiler (a cross build) the program still builds,
  // only without its icon.
  if let Err(e) = res.compile() {
    println!("cargo:warning=no Windows resources: {e}");
  }
}
