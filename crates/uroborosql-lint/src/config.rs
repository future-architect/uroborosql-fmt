mod config_object;
mod config_store;

use std::path::{Path, PathBuf};

pub use config_object::*;

const DEFAULT_CONFIG_FILE_NAME: &str = ".uroborosqllintrc.json";

pub fn search_upward_and_get_nearest_config(path: &Path) -> Option<PathBuf> {
    let mut current = path;
    while let Some(parent) = current.parent() {
        let config_file = parent.join(DEFAULT_CONFIG_FILE_NAME);
        if config_file.exists() {
            return Some(config_file);
        }
        current = parent;
    }
    None
}

#[cfg(test)]
mod search_upward_tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn found() {
        let temp_dir = TempDir::new().unwrap();

        // temp_dir/
        //   .uroborosqllintrc.json (config file)
        //   subdir1/
        //     subdir2/
        //       ^ search from here
        let config_file = temp_dir.path().join(DEFAULT_CONFIG_FILE_NAME);
        let subdir1 = temp_dir.path().join("subdir1");
        let subdir2 = subdir1.join("subdir2");

        fs::create_dir_all(&subdir2).unwrap();
        fs::write(&config_file, "{}").unwrap();

        let result = search_upward_and_get_nearest_config(&subdir2);
        assert_eq!(result, Some(config_file));
    }

    #[test]
    fn not_found() {
        let temp_dir = TempDir::new().unwrap();

        // no config file:
        // temp_dir/
        //   subdir1/
        //     subdir2/
        //       ^ search from here
        let subdir1 = temp_dir.path().join("subdir1");
        let subdir2 = subdir1.join("subdir2");
        fs::create_dir_all(&subdir2).unwrap();

        let result = search_upward_and_get_nearest_config(&subdir2);
        assert_eq!(result, None);
    }

    #[test]
    fn finds_nearest() {
        let temp_dir = TempDir::new().unwrap();

        // multiple config files:
        // temp_dir/
        //   .uroborosqllintrc.json (far config)
        //   subdir1/
        //     .uroborosqllintrc.json (near config)
        //     subdir2/
        //       ^ search from here
        let far_config = temp_dir.path().join(DEFAULT_CONFIG_FILE_NAME);
        let subdir1 = temp_dir.path().join("subdir1");
        let subdir2 = subdir1.join("subdir2");
        let near_config = subdir1.join(DEFAULT_CONFIG_FILE_NAME);

        fs::create_dir_all(&subdir2).unwrap();
        fs::write(&far_config, "{}").unwrap();
        fs::write(&near_config, "{}").unwrap();

        let result = search_upward_and_get_nearest_config(&subdir2);
        assert_eq!(result, Some(near_config));
    }

    // #[test]
    // fn finds_config_in_current_dir() {
    //     let temp_dir = TempDir::new().unwrap();

    //     // config file in current directory:
    //     // temp_dir/
    //     //   .uroborosqllintrc.json (not target)
    //     //   subdir1/
    //     //     subdir2/ <--- search from here
    //     //       .uroborosqllintrc.json (current dir config)
    //     let not_target_config = temp_dir.path().join(DEFAULT_CONFIG_FILE_NAME);
    //     let subdir1 = temp_dir.path().join("subdir1");
    //     let subdir2 = subdir1.join("subdir2");
    //     let current_dir_config = subdir2.join(DEFAULT_CONFIG_FILE_NAME);

    //     fs::create_dir_all(&subdir2).unwrap();
    //     fs::write(&not_target_config, "{}").unwrap();
    //     fs::write(&current_dir_config, "{}").unwrap();

    //     let result = search_upward_and_get_nearest_config(&subdir2);
    //     assert_eq!(result, Some(current_dir_config));
    // }
}
