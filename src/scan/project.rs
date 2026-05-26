use std::path::PathBuf;

pub fn infer_project_path(cmdline: &str, executable: &str) -> Option<String> {
    for token in split_cmd_tokens(cmdline) {
        if let Some(dir) = directory_from_path_token(&token) {
            return Some(dir);
        }
    }

    if let Some(dir) = directory_from_path_token(executable) {
        return Some(dir);
    }

    None
}

fn directory_from_path_token(token: &str) -> Option<String> {
    let token = token.trim_matches('"');
    if is_windows_style_path(token) {
        return parent_windows_path(token);
    }
    if token.starts_with('/') {
        return parent_unix_path(token);
    }
    None
}

fn is_windows_style_path(token: &str) -> bool {
    let path = token.replace('/', "\\");
    if path.starts_with("\\\\?\\") {
        return true;
    }
    if path.len() >= 3 && path.as_bytes()[1] == b':' && path.as_bytes()[2] == b'\\' {
        return true;
    }
    path.starts_with("\\\\")
}

fn parent_windows_path(token: &str) -> Option<String> {
    let path = token.replace('/', "\\");
    let path = path
        .strip_prefix("\\\\?\\UNC\\")
        .map(|rest| format!("\\\\{rest}"))
        .or_else(|| path.strip_prefix("\\\\?\\").map(str::to_string))
        .unwrap_or(path);

    let path = path.trim_end_matches('\\');
    let (dir, leaf) = path.rsplit_once('\\')?;
    if dir.is_empty() || leaf.is_empty() {
        return None;
    }
    Some(dir.to_string())
}

fn parent_unix_path(token: &str) -> Option<String> {
    let path = PathBuf::from(token);
    if path.is_file() {
        return path.parent().map(|p| p.display().to_string());
    }
    if path.is_dir() {
        return Some(path.display().to_string());
    }
    if path.extension().is_some() {
        return path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(|p| p.display().to_string());
    }
    None
}

fn split_cmd_tokens(cmdline: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in cmdline.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                current.push(ch);
            }
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infers_from_script_path() {
        let cmd = r#"node.exe "D:\dev\my-app\server.js""#;
        let dir = infer_project_path(cmd, "node.exe");
        assert_eq!(dir.as_deref(), Some(r"D:\dev\my-app"));
    }

    #[test]
    fn infers_from_csproj() {
        let cmd = r#"dotnet.exe run --project D:\dev\api\Web.csproj"#;
        let dir = infer_project_path(cmd, "dotnet.exe");
        assert_eq!(dir.as_deref(), Some(r"D:\dev\api"));
    }

    #[test]
    fn infers_from_unix_script_path() {
        let cmd = "node /home/runner/work/app/server.js";
        let dir = infer_project_path(cmd, "node");
        assert_eq!(dir.as_deref(), Some("/home/runner/work/app"));
    }

    #[test]
    fn ignores_bare_flags() {
        assert!(infer_project_path("node --watch", "node.exe").is_none());
    }
}
