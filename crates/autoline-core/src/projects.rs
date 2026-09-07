use std::path::{Path, PathBuf};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectId(pub String);

pub fn detect_project(cwd: &str) -> Option<ProjectId> {
    let path = Path::new(cwd);
    
    // Walk up the directory tree to find a project marker
    let mut current = path;
    loop {
        if current.join(".git").exists() || current.join(".autoline.project").exists() {
            let project_path = current.to_path_buf();
            return Some(ProjectId(hash_path(&project_path)));
        }
        
        match current.parent() {
            Some(parent) => current = parent,
            None => break,
        }
    }
    
    None
}

fn hash_path(path: &PathBuf) -> String {
    let mut s = DefaultHasher::new();
    path.hash(&mut s);
    format!("{:x}", s.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn detects_git_root() {
        let dir = tempdir().unwrap();
        let project_root = dir.path().join("project");
        fs::create_dir(&project_root).unwrap();
        fs::create_dir(project_root.join(".git")).unwrap();
        let nested_dir = project_root.join("src/nested");
        fs::create_dir_all(&nested_dir).unwrap();

        let project_id = detect_project(nested_dir.to_str().unwrap());
        assert!(project_id.is_some());
    }

    #[test]
    fn detects_autoline_project_marker() {
        let dir = tempdir().unwrap();
        let project_root = dir.path().join("project");
        fs::create_dir(&project_root).unwrap();
        fs::File::create(project_root.join(".autoline.project")).unwrap();
        let nested_dir = project_root.join("src/nested");
        fs::create_dir_all(&nested_dir).unwrap();

        let project_id = detect_project(nested_dir.to_str().unwrap());
        assert!(project_id.is_some());
    }

    #[test]
    fn no_project_detected() {
        let dir = tempdir().unwrap();
        let project_id = detect_project(dir.path().to_str().unwrap());
        assert!(project_id.is_none());
    }
}
