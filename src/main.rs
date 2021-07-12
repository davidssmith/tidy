use std::collections::HashMap;
use std::fmt::{self, Display};
use std::fs;
use std::iter::Iterator;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::Result;
use md5::{self, Digest};
use rayon::prelude::*;
use structopt::StructOpt;
use walkdir::WalkDir;


#[derive(Debug, Clone)]
struct Directory {
    path: PathBuf,
    files: Vec<PathBuf>,
    dirs: Vec<PathBuf>,
}


impl Directory {
    fn read(path: PathBuf) -> Result<Directory> {
        let mut files = Vec::new();
        let mut dirs = Vec::new();
        for entry in fs::read_dir(&path)? { 
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                dirs.push(entry.path().canonicalize()?);
            } else if entry.file_type()?.is_file() {
                files.push(entry.path().canonicalize()?);
            }  /* ignore links for now */
		}
        Ok(Directory { path, files, dirs })
    }
    fn is_empty(&self) -> bool {
        self.files.is_empty() && self.dirs.is_empty()
    }
}


impl Display for Directory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "=== {} ===\nFILES{:?}\nDIRS{:?} ", 
            self.path.display(), self.files, self.dirs)
    }
}


#[derive(Debug, Clone)]
struct DirTree {
    root: PathBuf,
    dirs: Vec<Directory>,
}


impl DirTree {
    fn read(root: PathBuf) -> Result<DirTree> {
        let mut dirs = Vec::new();
        //let mut dirs_seen: HashMap::<PathBuf, Directory> = HashMap::new();
        let mut dirs_left = vec![root.clone()];
        while !dirs_left.is_empty() {
            let next_dir = dirs_left.pop()
                .expect("This should never happen.");
            let cur = Directory::read(next_dir)?;
            dirs_left.extend_from_slice(&cur.dirs[..]);
            dirs.push(cur);
        }
        Ok(DirTree { root, dirs })
    }
    fn files(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();
        for dir in self.dirs.iter() {
            files.extend_from_slice(&dir.files[..]);
        }
        files
    }
    fn dirs(&self) -> Vec<PathBuf> {
        self.dirs.iter().map(|d| d.path.clone()).collect()
    }
    fn empty_dirs(&self) -> Vec<PathBuf> {
        self.dirs.iter()
            .filter(|d| d.is_empty())
            .map(|d| d.path.clone())
            .collect()
    }
    fn hash_dict(&self) -> HashMap<Digest, Vec<PathBuf>> {
		let mut hash_dict: HashMap<Digest, Vec<PathBuf>> = HashMap::new();
        let files: Vec<PathBuf> = self.files();
		let hashes: Vec<Digest> = files.par_iter()
            .map(|f| 
                md5::compute(&fs::read(f).unwrap())
            ).collect();
        for (h, p) in hashes.iter().zip(files.iter()) {
            match hash_dict.get_mut(&h) {
                Some(x) => x.push(p.clone()),
                None => {
                    hash_dict.insert(*h, vec![p.clone()]);
                },
            }
        }
		hash_dict
    }
}


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

    while !dirs_left.is_empty() { /* TODO: parallelize this? */
        let next_dir = dirs_left.pop().unwrap();
        for entry in fs::read_dir(next_dir)? { /* TODO: map + collect */
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

fn all_files3(root: PathBuf) -> Result<Vec<PathBuf>> {
    let dt = DirTree::read(root)?;
    for d in dt.empty_dirs().iter() {
        println!("EMPTY: {}", d.display());
    }
    let hd = dt.hash_dict();
    println!("hashes: {:?}", hd.len());
    for v in hd.values() {
        if v.len() > 1 {
            println!("{:?}", v);
        }
    }
    //println!("duplications: {:?}", dupes);

    Ok(dt.files())
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



fn main() -> Result<()> {
    let opt = Opt::from_args();
    println!("{:#?}", opt);
    for p in opt.paths.iter() {
        let now = Instant::now();
        let files = all_files1(&p);
        let telap = now.elapsed().as_millis();
        println!("#1 {} => {} files ({} ms elapsed)", p.display(), files.len(), telap);
        let now = Instant::now();
        let files = all_files2(p.to_path_buf())?;
        let telap = now.elapsed().as_millis();
        println!("#2 {} => {} files ({} ms elapsed)", p.display(), files.len(), telap);
        let now = Instant::now();
        let files = all_files3(p.to_path_buf())?;
        let telap = now.elapsed().as_millis();
        println!("#3 {} => {} files ({} ms elapsed)", p.display(), files.len(), telap);
        //visit_dirs(p, &md5_sum);
        //let now = Instant::now();
        //files.par_iter().map(|f| md5_sum(&f)).collect::<Vec<String>>(); 
        //let telap = now.elapsed().as_millis();
        //println!("Par MD5: {} ms elapsed", telap);
    }
    Ok(())
}

