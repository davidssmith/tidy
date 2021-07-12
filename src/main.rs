//use std::collections::HashMap;
//use std::fmt::{self, Display};
use std::fs::{self, DirEntry};
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::Result;
//use md5;
//use rayon;
use structopt::StructOpt;
use walkdir::WalkDir;

fn all_files1(root: impl AsRef<Path>) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in WalkDir::new(&root).follow_links(false) {
        if let Ok(e) = entry {
            if e.file_type().is_file() {
                //println!("{}", e.path().display());
                files.push(e.path().into());
            }
        }
    }
    files
}


fn all_files2(root: PathBuf) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut dirs_left = vec![root.clone()];
    let follow_links = false;
    while !dirs_left.is_empty() {
        let next_dir = dirs_left.pop().unwrap();
        for entry in fs::read_dir(next_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                dirs_left.push(entry.path());
            } else if entry.file_type()?.is_file() {
                files.push(entry.path());
            } else if follow_links { /* symlink */
                let s = fs::read_link(entry.path());
                if s.is_err() {
                    continue;
                }
                let s = s.unwrap();
                if !s.starts_with(&root) { /* don't cross outside root boundary */
                    continue;
                }
                if s.is_dir() {
                    dirs_left.push(s);
                } else {
                    files.push(s);
                }
            } 
        //}
        }
    }
    Ok(files)
}

fn visit_dirs(dir: &Path, cb: &dyn Fn(&DirEntry)) -> Result<()> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() && !entry.file_type()?.is_symlink() {
                visit_dirs(&path, cb)?;
            } else {
                cb(&entry);
            }
        }
    }
    Ok(())
}


/// A basic example
#[derive(StructOpt, Debug)]
#[structopt(name = "Tidy")]
struct Opt {
    // A flag, true if used in the command line. Note doc comment will
    // be used for the help message of the flag. The name of the
    // argument will be, by default, based on the name of the field.
    /// Dry run (no actions will be applied)
    #[structopt(long)]
    dry_run: bool,

    /// Deduplicate files
    #[structopt(short = "D", long)]
    dedup: bool,

    /// Trim small directories
    #[structopt(short = "T", long)]
    trim: bool,

    /// Set maximum directory size to trim
    #[structopt(short, long, default_value = "0")]
    trim_max: u64,

    /// Directory roots to process
    #[structopt(name = "INPUT", parse(from_os_str))]
    paths: Vec<PathBuf>,
}


fn md5_sum(d: &DirEntry) {
    if let Ok(b) = fs::read(d.path()) {
        println!("{} => {:?}", d.path().display(), md5::compute(&b));
    }
}


fn main() -> Result<()> {
    let opt = Opt::from_args();
    println!("{:#?}", opt);
    for p in opt.paths.iter() {
        let now = Instant::now();
        let f = all_files1(&p);
        let telap = now.elapsed().as_micros();
        println!("#1 {} => {} files ({:.3} s elapsed)", p.display(), f.len(), telap);
        let now = Instant::now();
        let f = all_files2(p.to_path_buf())?;
        let telap = now.elapsed().as_micros();
        println!("#2 {} => {} files ({:.3} s elapsed)", p.display(), f.len(), telap);
        //visit_dirs(p, &md5_sum);
    }
    Ok(())
}

