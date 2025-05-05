use std::path::PathBuf;

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
pub fn clip_path(folder_path: PathBuf, file_path: PathBuf, desired_length: Option<usize>) -> String {
    // AI generated method, DeepThink (R1)

    let folder_components: Vec<_> = folder_path.components().collect();
    let file_components: Vec<_> = file_path.components().collect();

    let common_prefix = folder_components
        .iter()
        .zip(file_components.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let sep = std::path::MAIN_SEPARATOR.to_string();
    let sep_str = sep.as_str();

    let file_components_str: Vec<String> = file_components
        .iter()
        .map(|c| {
            c.as_os_str()
                .to_string_lossy()
                .into_owned()
        })
        .collect();

    if common_prefix == folder_components.len() {
        let difference_components_str = &file_components_str[common_prefix..];
        if difference_components_str.is_empty() {
            return String::new();
        }

        if let Some(max_len) = desired_length {
            if difference_components_str.len() <= 2 {
                return difference_components_str.join(sep_str);
            }

            let mut best_candidate = String::new();

            for k in (1..=difference_components_str.len()).rev() {
                let start_index = difference_components_str
                    .len()
                    .saturating_sub(k);
                let suffix = &difference_components_str[start_index..];
                let candidate = format!("...{}{}", sep_str, suffix.join(sep_str));
                if candidate.len() <= max_len {
                    best_candidate = candidate;
                    break;
                }
            }

            if best_candidate.is_empty() {
                let minimal_candidate = format!(
                    "...{}{}{}{}",
                    sep_str,
                    difference_components_str[difference_components_str.len() - 2],
                    sep_str,
                    difference_components_str
                        .last()
                        .unwrap()
                );
                best_candidate = if minimal_candidate.len() <= max_len {
                    minimal_candidate
                } else {
                    format!("...{}", sep_str)
                };
            }

            best_candidate
        } else {
            if difference_components_str.len() <= 2 {
                difference_components_str.join(sep_str)
            } else {
                format!(
                    "...{}{}{}{}",
                    sep_str,
                    difference_components_str[difference_components_str.len() - 2],
                    sep_str,
                    difference_components_str
                        .last()
                        .unwrap()
                )
            }
        }
    } else {
        if file_components_str.is_empty() {
            return String::new();
        }

        let default_clipped = if file_components_str.len() <= 2 {
            file_components_str.join(sep_str)
        } else {
            format!(
                "{}{}...{}{}{}{}",
                file_components_str[0],
                sep_str,
                sep_str,
                file_components_str[file_components_str.len() - 2],
                sep_str,
                file_components_str.last().unwrap()
            )
        };

        if let Some(max_len) = desired_length {
            if default_clipped.len() <= max_len {
                default_clipped
            } else {
                if file_components_str.len() >= 2 {
                    format!(
                        "{}{}...{}{}",
                        file_components_str[0],
                        sep_str,
                        sep_str,
                        file_components_str.last().unwrap()
                    )
                } else {
                    default_clipped
                }
            }
        } else {
            default_clipped
        }
    }
}

#[cfg(test)]
mod test {
    // Human generated test cases to match human generated method description.
    use super::*;

    #[test]
    #[cfg_attr(not(target_os = "windows"), ignore)]
    pub fn clip_with_same_parent_folder() {
        let folder_path = PathBuf::from(r#"D:\Users\Hydra\Project1"#);
        let file_path_1 = PathBuf::from(r#"D:\Users\Hydra\Project1\file.mpnp"#);
        let clipped_file_path1: String = clip_path(folder_path, file_path_1, None);
        assert_eq!(clipped_file_path1, r#"file.mpnp"#);
    }

    #[test]
    #[cfg_attr(not(target_os = "windows"), ignore)]
    pub fn clip_with_same_parent_folder_but_longer_path_to_file() {
        let folder_path = PathBuf::from(r#"D:\Users\Hydra\Project1"#);
        let file_path_1 = PathBuf::from(r#"D:\Users\Hydra\Project1\additional\path\entries\file.mpnp"#);
        let clipped_file_path1: String = clip_path(folder_path, file_path_1, None);
        assert_eq!(clipped_file_path1, r#"...\entries\file.mpnp"#);
    }

    #[test]
    #[cfg_attr(not(target_os = "windows"), ignore)]
    pub fn clip_with_different_parent_folder() {
        let folder_path = PathBuf::from(r#"C:\Some\Other\Folder"#);
        let file_path_2 = PathBuf::from(r#"D:\Users\Hydra\Project1\file.mpnp"#);
        let clipped_file_path2: String = clip_path(folder_path, file_path_2, None);
        assert_eq!(clipped_file_path2, r#"D:\...\Project1\file.mpnp"#);
    }

    #[test]
    #[cfg_attr(not(target_os = "windows"), ignore)]
    fn clip_with_nested_file_and_short_length() {
        let folder_path = PathBuf::from(r#"D:\Users\Hydra\Project1"#);
        let file_path = PathBuf::from(r#"D:\Users\Hydra\Project1\src\utils\mod.rs"#);
        let clipped = clip_path(folder_path, file_path, Some(18));
        assert_eq!(clipped, r#"...\utils\mod.rs"#);
    }

    #[test]
    #[cfg_attr(not(target_os = "windows"), ignore)]
    fn clip_with_nested_file_and_long_length() {
        let folder_path = PathBuf::from(r#"D:\Users\Hydra\Project1"#);
        let file_path = PathBuf::from(r#"D:\Users\Hydra\Project1\src\utils\mod.rs"#);
        let clipped = clip_path(folder_path, file_path, Some(21));
        assert_eq!(clipped, r#"...\src\utils\mod.rs"#);
    }

    #[test]
    #[cfg_attr(not(target_os = "windows"), ignore)]
    fn clip_with_nested_file_and_result_matching_exact_desired_length() {
        let folder_path = PathBuf::from(r#"D:\Users\Hydra\Project1"#);
        let file_path = PathBuf::from(r#"D:\Users\Hydra\Project1\src\utils\mod.rs"#);
        let clipped = clip_path(folder_path, file_path, Some(20));
        assert_eq!(clipped, r#"...\src\utils\mod.rs"#);
    }

    #[test]
    #[cfg_attr(not(target_os = "windows"), ignore)]
    pub fn clip_with_different_parent_folder_and_desired_length() {
        let folder_path = PathBuf::from(r#"C:\Some\Other\Folder"#);
        let file_path_2 = PathBuf::from(r#"D:\Users\Hydra\Project1\file.mpnp"#);
        let clipped_file_path2: String = clip_path(folder_path, file_path_2, Some(27));
        assert_eq!(clipped_file_path2, r#"D:\...\Project1\file.mpnp"#);
    }

    #[test]
    #[cfg_attr(not(target_os = "windows"), ignore)]
    pub fn real_world_test_1() {
        let folder_path = PathBuf::from(r#"D:\Users\Hydra\Documents\dev\projects\makerpnp\projects\Test1"#);
        let file_path_2 = PathBuf::from(r#"D:\Users\Hydra\Documents\dev\projects\makerpnp\projects\Test1\loadout.csv"#);
        let clipped_file_path2: String = clip_path(folder_path, file_path_2, None);
        assert_eq!(clipped_file_path2, r#"loadout.csv"#);
    }

    #[test]
    #[cfg_attr(not(target_os = "windows"), ignore)]
    pub fn real_world_test_2() {
        let folder_path = PathBuf::from(r#"D:\Users\Hydra\Documents\dev\projects\makerpnp\projects\Test1"#);
        let file_path_2 = PathBuf::from(r#"D:\Users\Hydra\Documents\dev\projects\makerpnp\projects\Test1\loadout.csv"#);
        let clipped_file_path2: String = clip_path(folder_path, file_path_2, Some(27));
        assert_eq!(clipped_file_path2, r#"loadout.csv"#);
    }
}
