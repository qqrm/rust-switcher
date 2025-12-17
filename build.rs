fn main() {
    // Compile the resource file for Windows so that the executable
    // includes version information and an icon.  The original project
    // ships a small resource script (`app.rc`) and manifest which
    // embed metadata into the binary.  This call is a no‑op on
    // non‑Windows targets.
    let _ = embed_resource::compile("app.rc", embed_resource::NONE);
}