//! `cargo run -p arctic-mesh --example check -- <file.glb>...`: what the server would say about each file.

fn main() {
    for path in std::env::args().skip(1) {
        let bytes = std::fs::read(&path).expect("readable file");
        match arctic_mesh::parse(&bytes) {
            Ok(m) => {
                let (lo, hi) = m.bounds();
                println!(
                    "{path}: ok, {} triangles, {} nodes ({}), {} primitives, {} materials, {} images, animation {:?}, bounds {lo:?} to {hi:?}",
                    m.triangles(),
                    m.nodes.len(),
                    m.nodes.iter().map(|n| n.name.as_str()).collect::<Vec<_>>().join("/"),
                    m.primitives.len(),
                    m.materials.len(),
                    m.images.len(),
                    m.animation.as_ref().map(|a| a.length),
                );
            }
            Err(e) => println!("{path}: REFUSED: {e}"),
        }
    }
}
