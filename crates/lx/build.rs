fn main() -> anyhow::Result<()> {
    // Embed schemas at compile time (includes lexiang.json + unlisted.json)
    println!("cargo:rerun-if-changed=../../schemas/");
    println!("cargo:rerun-if-changed=../../schemas/unlisted.json");

    // Watch static skill files for recompilation
    println!("cargo:rerun-if-changed=../../skills/");
    Ok(())
}
