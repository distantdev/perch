use crate::model::ServerType;

pub fn classify_server(executable: &str, cmdline: &str) -> ServerType {
    let exe_lower = executable.to_ascii_lowercase();
    let cmd_lower = cmdline.to_ascii_lowercase();
    let haystack = format!("{exe_lower} {cmd_lower}");

    if is_docker_proxy(&exe_lower) {
        return ServerType::Docker;
    }

    if haystack.contains("node")
        || haystack.contains("npm")
        || haystack.contains("vite")
        || haystack.contains("next")
        || haystack.contains("npx")
        || haystack.contains("bun")
    {
        return ServerType::Node;
    }

    if haystack.contains("dotnet") {
        return ServerType::DotNet;
    }

    if haystack.contains("python")
        || haystack.contains("uvicorn")
        || haystack.contains("gunicorn")
        || haystack.contains("django")
        || haystack.contains("flask")
    {
        return ServerType::Python;
    }

    ServerType::Other
}

pub fn is_likely_dev_server(server_type: ServerType, executable: &str, cmdline: &str) -> bool {
    match server_type {
        ServerType::Node | ServerType::DotNet | ServerType::Python | ServerType::Docker => true,
        ServerType::Other => looks_like_dev_process(executable, cmdline),
    }
}

fn looks_like_dev_process(executable: &str, cmdline: &str) -> bool {
    let haystack = format!(
        "{} {}",
        executable.to_ascii_lowercase(),
        cmdline.to_ascii_lowercase()
    );

    const DEV_EXECUTABLES: &[&str] = &[
        "node",
        "npm",
        "npx",
        "yarn",
        "pnpm",
        "bun",
        "deno",
        "dotnet",
        "python",
        "python3",
        "py.exe",
        "ruby",
        "rails",
        "java",
        "gradle",
        "mvn",
        "go",
        "cargo",
        "rustc",
        "php",
        "composer",
        "vite",
        "webpack",
        "ng ",
        "next",
        "http-server",
        "live-server",
        "serve",
        "postgres",
        "mysql",
        "mysqld",
        "redis",
        "mongod",
        "docker",
        "podman",
        "nginx",
        "caddy",
        "httpd",
        "esbuild",
        "turbo",
        "expo",
        "metro",
        "webpack-dev-server",
        "parcel",
        "storybook",
        "minio",
        "traefik",
        "air",
    ];

    if DEV_EXECUTABLES
        .iter()
        .any(|needle| haystack.contains(needle))
    {
        return true;
    }

    const DEV_CMDLINE: &[&str] = &[
        " run dev",
        " run serve",
        " run start",
        " dev",
        " serve",
        " watch",
        " listen",
        "next dev",
        "ng serve",
        "rails server",
        "rails s",
        "dotnet run",
        "dotnet watch",
        "cargo run",
        "npm run",
        "pnpm run",
        "yarn run",
        "localhost",
        "--port",
        "-p ",
        "http.server",
        "uvicorn",
        "gunicorn",
        "flask",
        "django",
        "spring-boot",
        "gradle bootrun",
        "create-react-app",
    ];

    DEV_CMDLINE.iter().any(|needle| haystack.contains(needle))
}

pub fn is_docker_proxy(executable: &str) -> bool {
    let exe = executable.to_ascii_lowercase();
    exe.contains("docker-proxy")
        || exe.contains("com.docker")
        || exe == "vpnkit"
        || exe.contains("docker desktop")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_node() {
        assert_eq!(
            classify_server("node.exe", "node server.js"),
            ServerType::Node
        );
    }

    #[test]
    fn classifies_dotnet() {
        assert_eq!(
            classify_server("dotnet", "dotnet run --project api"),
            ServerType::DotNet
        );
    }

    #[test]
    fn classifies_python() {
        assert_eq!(
            classify_server("python3", "python -m uvicorn main:app"),
            ServerType::Python
        );
    }

    #[test]
    fn classifies_docker_proxy() {
        assert_eq!(classify_server("docker-proxy", ""), ServerType::Docker);
    }

    #[test]
    fn classifies_other() {
        assert_eq!(classify_server("ruby", "rails s"), ServerType::Other);
    }

    #[test]
    fn dev_heuristic_rails() {
        assert!(is_likely_dev_server(
            ServerType::Other,
            "ruby",
            "rails server -p 3000"
        ));
    }

    #[test]
    fn dev_heuristic_rejects_svchost() {
        assert!(!is_likely_dev_server(
            ServerType::Other,
            "svchost.exe",
            "C:\\Windows\\System32\\svchost.exe -k netsvcs"
        ));
    }
}
