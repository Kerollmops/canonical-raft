use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{env, io, fs, thread, process};
use rand::Rng;

// Number of servers in the example cluster
const N_SERVERS: usize = 3;

fn spawn_child(top_level_dir: &Path, id: usize) -> io::Result<process::Child> {
    let id = id + 1;

    let command = concat!(env!("CARGO_MANIFEST_DIR"), "/target/release/examples/server");
    let id = id.to_string();
    let dir = top_level_dir.join(&id);

    fs::create_dir_all(&dir);

    let dir = dir.display().to_string();
    process::Command::new(command).args(&[&dir, &id]).spawn()
}

fn main() {
    let mut top_level_dir = PathBuf::from("/tmp/raft");
    let mut children = Vec::new();

    assert!(N_SERVERS != 0);

    if let Some(dir) = env::args().nth(1) {
        top_level_dir = PathBuf::from(dir);
    }

    // Make sure the top level directory exists.
    fs::create_dir_all(&top_level_dir);

    // Spawn the cluster nodes
    for i in 0..N_SERVERS {
        let child = spawn_child(&top_level_dir, i).unwrap();
        children.push(Some(child));
    }

    // Create the random generator
    let mut rng = rand::thread_rng();

    loop {
        // Sleep a little bit.
        let duration = Duration::from_secs(rng.gen_range(1, 16));
        thread::sleep(duration);

        // Select and kill a random server.
        let i = rng.gen_range(0, N_SERVERS);
        let child = children.get_mut(i).unwrap();
        child.take().unwrap().kill().unwrap();

        println!("Killed server {}", i + 1);

        thread::sleep(duration);

        *child = Some(spawn_child(&top_level_dir, i).unwrap());
    }
}
