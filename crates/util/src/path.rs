/// Clip a path to a given length.
///
/// * If the start of the file_path is different to the start of the folder_path, the first element in
///   the file_path is returned, followed by '...' or as many remaining elements of the file_path as possible,
///   followed by the final element of the file path.
///   Additionally in this case, when '...' is used will always be surrounded by the OS native path separator. e.g. '<leading_path><separator>...<separator><remaining_path>'
/// * If the start of the file_path is the same as the start of the folder_path then the difference between the 
///   file_path and folder_path is returned.
///   In this case '...' can be used to replace as many elements of the returned file path as possible, followed by the last
///   two elements of the file path.
///   Additionally in this case, when '...' is used will be followed by the OS native path separator. e.g. '...<separator><remaining_path>'
/// * If desired_length is specified, then the returned string should be shortened, using the above rules
///   so that it contains the required elements and '...' replaces as few elements as possible in order to achieve
///   the desired length.
use std::path::{PathBuf, Component};


pub fn clip_path(folder_path: PathBuf, file_path: PathBuf, desired_length: Option<usize>) -> String {
    todo!()
}

mod test {
    use super::*;
    
    #[test]
    pub fn clip_with_same_parent_folder() {
        let folder_path = PathBuf::from(r#"D:\Users\Hydra\Project1"#);
        let file_path_1 = PathBuf::from(r#"D:\Users\Hydra\Project1\file.mpnp"#);
        let clipped_file_path1: String = clip_path(folder_path, file_path_1, None);
        assert_eq!(clipped_file_path1, r#"file.mpnp"#);
    }

    #[test]
    pub fn clip_with_same_parent_folder_but_longer_path_to_file() {
        let folder_path = PathBuf::from(r#"D:\Users\Hydra\Project1"#);
        let file_path_1 = PathBuf::from(r#"D:\Users\Hydra\Project1\additional\path\entries\file.mpnp"#);
        let clipped_file_path1: String = clip_path(folder_path, file_path_1, None);
        assert_eq!(clipped_file_path1, r#"...\entries\file.mpnp"#);
    }

    #[test]
    pub fn clip_with_different_parent_folder() {
        let folder_path = PathBuf::from(r#"C:\Some\Other\Folder"#);
        let file_path_2 = PathBuf::from(r#"D:\Users\Hydra\Project1\file.mpnp"#);
        let clipped_file_path2: String = clip_path(folder_path, file_path_2, None);
        assert_eq!(clipped_file_path2, r#"D:\...\Project1\file.mpnp"#);
    }

    #[test]
    fn clip_with_nested_file_and_short_length() {
        let folder_path = PathBuf::from(r#"D:\Users\Hydra\Project1"#);
        let file_path = PathBuf::from(r#"D:\Users\Hydra\Project1\src\utils\mod.rs"#);
        let clipped = clip_path(folder_path, file_path, Some(18));
        assert_eq!(clipped, r#"...\utils\mod.rs"#);
    }
    
    #[test]
    fn clip_with_nested_file_and_long_length() {
        let folder_path = PathBuf::from(r#"D:\Users\Hydra\Project1"#);
        let file_path = PathBuf::from(r#"D:\Users\Hydra\Project1\src\utils\mod.rs"#);
        let clipped = clip_path(folder_path, file_path, Some(21));
        assert_eq!(clipped, r#"...\src\utils\mod.rs"#);
    }

    #[test]
    fn clip_with_nested_file_and_result_matching_exact_desired_length() {
        let folder_path = PathBuf::from(r#"D:\Users\Hydra\Project1"#);
        let file_path = PathBuf::from(r#"D:\Users\Hydra\Project1\src\utils\mod.rs"#);
        let clipped = clip_path(folder_path, file_path, Some(20));
        assert_eq!(clipped, r#"...\src\utils\mod.rs"#);
    }

    #[test]
    pub fn clip_with_different_parent_folder_and_desired_length() {
        let folder_path = PathBuf::from(r#"C:\Some\Other\Folder"#);
        let file_path_2 = PathBuf::from(r#"D:\Users\Hydra\Project1\file.mpnp"#);
        let clipped_file_path2: String = clip_path(folder_path, file_path_2, Some(27));
        assert_eq!(clipped_file_path2, r#"D:\...\Project1\file.mpnp"#);
    }
}